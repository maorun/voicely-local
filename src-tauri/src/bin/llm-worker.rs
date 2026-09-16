use std::{collections::HashSet, env, io::{self, BufRead, Write}, num::NonZeroU32, path::PathBuf, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}, thread};
use llama_cpp_2::{context::params::LlamaContextParams, llama_backend::LlamaBackend, llama_batch::LlamaBatch, model::{params::LlamaModelParams, AddBos, LlamaModel}, sampling::LlamaSampler};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)] #[serde(tag = "type", rename_all = "snake_case")]
enum Request { Generate { id: String, prompt: String, max_tokens: Option<i32> }, Cancel { id: String }, Shutdown }
#[derive(Serialize)] #[serde(tag = "type", rename_all = "snake_case")]
enum Response<'a> { Ready, Token { id: &'a str, token: &'a str }, Done { id: &'a str }, Error { id: &'a str, message: String }, Cancelled { id: &'a str } }

struct Cancellations { active: Mutex<Option<String>>, cancelled: Mutex<HashSet<String>> }

extern "C" { fn _exit(code: i32) -> !; }
/// Terminate immediately without running C++ static destructors. ggml's Metal
/// backend has a known crash (SIGABRT in ggml_metal_rsets_free) when its global
/// residency-set state is torn down during a normal process exit; skipping
/// destructors entirely via the raw POSIX `_exit` avoids it reliably.
fn exit_without_destructors() -> ! { unsafe { _exit(0) } }

fn emit(response: Response) { let mut stdout = io::stdout().lock(); let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap()); let _ = stdout.flush(); }

fn run_generate(model: &LlamaModel, backend: &LlamaBackend, id: &str, prompt: &str, max_tokens: i32, cancellations: &Arc<Cancellations>, active_flag: &Arc<AtomicBool>) {
    let context_params = LlamaContextParams::default().with_n_ctx(Some(NonZeroU32::new(4096).unwrap())).with_n_threads(8).with_n_threads_batch(8);
    let mut context = match model.new_context(backend, context_params) { Ok(context) => context, Err(error) => { emit(Response::Error { id, message: error.to_string() }); return; } };
    let tokens = match model.str_to_token(prompt, AddBos::Always) { Ok(tokens) => tokens, Err(error) => { emit(Response::Error { id, message: error.to_string() }); return; } };
    if tokens.len() >= 3500 { emit(Response::Error { id, message: "Der Text ist für die lokale Umformulierung zu lang. Bitte kürze ihn zuerst.".into() }); return; }
    let mut batch = LlamaBatch::new(4096, 1); let last = tokens.len().saturating_sub(1) as i32;
    for (position, token) in tokens.into_iter().enumerate() { if let Err(error) = batch.add(token, position as i32, &[0], position as i32 == last) { emit(Response::Error { id, message: error.to_string() }); return; } }
    if let Err(error) = context.decode(&mut batch) { emit(Response::Error { id, message: error.to_string() }); return; }
    let mut sampler = LlamaSampler::chain_simple([LlamaSampler::dist(42), LlamaSampler::greedy()]);
    let mut decoder = encoding_rs::UTF_8.new_decoder(); let mut produced = false;
    active_flag.store(true, Ordering::SeqCst);
    for position in 0..max_tokens.max(1) {
        if cancellations.cancelled.lock().unwrap().remove(id) { active_flag.store(false, Ordering::SeqCst); emit(Response::Cancelled { id }); return; }
        let token = sampler.sample(&context, batch.n_tokens() - 1); sampler.accept(token); if model.is_eog_token(token) { break; }
        let piece = match model.token_to_piece(token, &mut decoder, true, None) { Ok(piece) => piece, Err(error) => { active_flag.store(false, Ordering::SeqCst); emit(Response::Error { id, message: error.to_string() }); return; } };
        if !piece.is_empty() { produced = true; emit(Response::Token { id, token: &piece }); }
        batch.clear(); if let Err(error) = batch.add(token, last + position + 1, &[0], true) { active_flag.store(false, Ordering::SeqCst); emit(Response::Error { id, message: error.to_string() }); return; }
        if let Err(error) = context.decode(&mut batch) { active_flag.store(false, Ordering::SeqCst); emit(Response::Error { id, message: error.to_string() }); return; }
    }
    active_flag.store(false, Ordering::SeqCst);
    if produced { emit(Response::Done { id }); } else { emit(Response::Error { id, message: "Das lokale Modell hat keine Ausgabe erzeugt.".into() }); }
}

// The request loop terminates exclusively through `_exit()` to avoid ggml Metal
// static-destructor crashes. Rust correctly sees the trailing Result as unreachable.
#[allow(unreachable_code)]
fn main() -> Result<(), String> {
    let model_path = PathBuf::from(env::args().nth(1).ok_or("Modellpfad fehlt.")?);
    let backend = LlamaBackend::init().map_err(|e| e.to_string())?; let params = LlamaModelParams::default().with_n_gpu_layers(99);
    let model = Arc::new(LlamaModel::load_from_file(&backend, &model_path, &params).map_err(|e| e.to_string())?);
    let backend = Arc::new(backend); let cancellations = Arc::new(Cancellations { active: Mutex::new(None), cancelled: Mutex::new(HashSet::new()) }); let active_flag = Arc::new(AtomicBool::new(false));
    emit(Response::Ready);
    let stdin = io::stdin(); let mut reader = stdin.lock();
    let mut line = String::new();
    loop {
        line.clear(); let read = reader.read_line(&mut line).map_err(|e| e.to_string())?; if read == 0 { exit_without_destructors(); }
        let request: Request = match serde_json::from_str(line.trim()) { Ok(request) => request, Err(_) => continue };
        match request {
            Request::Shutdown => exit_without_destructors(),
            Request::Cancel { id } => { cancellations.cancelled.lock().unwrap().insert(id); }
            Request::Generate { id, prompt, max_tokens } => {
                // A single model/backend must only serve one request at a time;
                // parallel generations otherwise interleave responses and contend for Metal.
                if active_flag.swap(true, Ordering::SeqCst) { emit(Response::Error { id: &id, message: "Eine lokale Textaktion läuft bereits. Bitte warte oder brich sie ab.".into() }); continue; }
                *cancellations.active.lock().unwrap() = Some(id.clone());
                let model = model.clone(); let backend = backend.clone(); let cancellations = cancellations.clone(); let active_flag = active_flag.clone(); let limit = max_tokens.unwrap_or(512);
                thread::spawn(move || { run_generate(&model, &backend, &id, &prompt, limit, &cancellations, &active_flag); active_flag.store(false, Ordering::SeqCst); });
            }
        }
    }
    Ok(())
}
