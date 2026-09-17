use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::{HashMap, HashSet}, fs, io::{BufRead, BufReader, Read, Write}, path::PathBuf, process::{Command, Stdio}, sync::{mpsc::Sender, Arc, Mutex}, thread, time::{Duration, Instant}};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

const WHISPER_RESOURCE: &str = "resources/models/ggml-small.bin";
const ACTIVE_MODEL_FILE: &str = "active-text-model";
const ACTIVE_WHISPER_FILE: &str = "active-whisper-model";
const BUNDLED_WHISPER_ID: &str = "whisper-small";

struct ModelSpec { id: &'static str, name: &'static str, description: &'static str, size_mb: u32, min_bytes: u64, url: &'static str, file: &'static str, sha256: &'static str }
const TEXT_MODELS: &[ModelSpec] = &[
    ModelSpec { id: "qwen3-4b", name: "Qwen3 4B", description: "Schnell für Korrekturen, Ton und kurze Nachrichten", size_mb: 2_500, min_bytes: 2_000_000_000, url: "https://huggingface.co/bartowski/Qwen_Qwen3-4B-GGUF/resolve/main/Qwen_Qwen3-4B-Q4_K_M.gguf", file: "qwen3-4b-q4_k_m.gguf", sha256: "fbe1d5edd4ce802ae3ae7c7e4ab7d09789d697fdac1fc7929f8df4ca3c41bae3" },
    ModelSpec { id: "qwen3-8b", name: "Qwen3 8B", description: "Empfohlen für hochwertige deutschsprachige Umformulierungen", size_mb: 5_200, min_bytes: 4_000_000_000, url: "https://huggingface.co/bartowski/Qwen_Qwen3-8B-GGUF/resolve/main/Qwen_Qwen3-8B-Q4_K_M.gguf", file: "qwen3-8b-q4_k_m.gguf", sha256: "54fffa050078e984116639c83dfb64b5aa6d4cd474e018b076777c632bbccccd" },
    ModelSpec { id: "qwen3-14b", name: "Qwen3 14B", description: "Höchste Qualität, benötigt mehr Speicher und Rechenzeit", size_mb: 8_900, min_bytes: 7_000_000_000, url: "https://huggingface.co/bartowski/Qwen_Qwen3-14B-GGUF/resolve/main/Qwen_Qwen3-14B-Q4_K_M.gguf", file: "qwen3-14b-q4_k_m.gguf", sha256: "915913e22399475dbe6c968ac014d9f1fbe08975e489279aede9d5c7b2c98eb6" },
];

const WHISPER_MODELS: &[ModelSpec] = &[
    ModelSpec { id: "whisper-base", name: "Schnell — Whisper Base", description: "Geringste Latenz für kurze Notizen", size_mb: 142, min_bytes: 140_000_000, url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin", file: "ggml-base.bin", sha256: "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe" },
    ModelSpec { id: BUNDLED_WHISPER_ID, name: "Ausgewogen — Whisper Small", description: "In der App enthalten, sofort offline verfügbar", size_mb: 466, min_bytes: 460_000_000, url: "", file: "ggml-small.bin", sha256: "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b" },
    ModelSpec { id: "whisper-medium", name: "Präzise — Whisper Medium", description: "Beste Qualität für lange Diktate und Fachsprache", size_mb: 1_530, min_bytes: 1_500_000_000, url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin", file: "ggml-medium.bin", sha256: "6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208" },
];

#[derive(Deserialize)] #[serde(rename_all = "camelCase")]
struct TranscriptionRequest { wav_base64: String, language: Option<String> }
#[derive(Serialize)] #[serde(rename_all = "camelCase")]
struct EngineStatus { ready: bool, message: String }
#[derive(Serialize)] #[serde(rename_all = "camelCase")]
struct SystemStatus { whisper_ready: bool, whisper_worker_ready: bool, llm_worker_ready: bool, active_model_ready: bool, accessibility_granted: bool }
#[derive(Serialize)] #[serde(rename_all = "camelCase")]
struct TextModel { id: String, name: String, description: String, size_mb: u32, installed: bool, active: bool, installed_bytes: u64, partial_bytes: u64 }
#[derive(Serialize, Clone)] #[serde(rename_all = "camelCase")]
struct RewriteToken { request_id: String, token: String }
#[derive(Serialize, Clone)] #[serde(rename_all = "camelCase")]
struct DownloadProgress { model_id: String, downloaded_bytes: u64, total_bytes: Option<u64> }
#[derive(Default)]
struct DownloadState(Arc<Mutex<HashSet<String>>>);
const LLM_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

enum WorkerEvent { Token(String), Done, Error(String), Cancelled }

struct LlmWorkerHandle { child: Mutex<std::process::Child>, stdin: Mutex<std::process::ChildStdin>, model_path: PathBuf, last_used: Mutex<Instant> }
#[derive(Clone, Default)]
struct LlmWorkerState { active: Arc<Mutex<Option<Arc<LlmWorkerHandle>>>>, pending: Arc<Mutex<HashMap<String, Sender<WorkerEvent>>>> }
#[derive(Clone, Serialize)] #[serde(rename_all = "camelCase")]
struct ActivationStatus { model_id: String, phase: &'static str, message: String }

fn bundled_model_path(app: &AppHandle) -> Result<PathBuf, String> {
    let path = app.path().resource_dir().map_err(|e| format!("App-Ressourcen konnten nicht geöffnet werden: {e}"))?.join(WHISPER_RESOURCE);
    if !path.is_file() { return Err("Das integrierte Sprachmodell fehlt. Bitte die App erneut installieren.".into()); }
    Ok(path)
}
fn text_models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app.path().app_local_data_dir().map_err(|e| e.to_string())?.join("text-models");
    fs::create_dir_all(&directory).map_err(|e| format!("Modellordner konnte nicht erstellt werden: {e}"))?; Ok(directory)
}
fn spec(id: &str) -> Result<&'static ModelSpec, String> { TEXT_MODELS.iter().find(|model| model.id == id).ok_or_else(|| "Unbekanntes Textmodell.".into()) }
fn verification_marker_path(path: &std::path::Path) -> PathBuf { path.with_extension(format!("{}verified", path.extension().and_then(|value| value.to_str()).unwrap_or(""))) }
fn write_verification_marker(path: &std::path::Path, checksum: &str) -> Result<(), String> { let size = fs::metadata(path).map_err(|e| e.to_string())?.len(); fs::write(verification_marker_path(path), format!("{checksum}:{size}")) .map_err(|e| e.to_string()) }
fn has_verification_marker(path: &std::path::Path, checksum: &str) -> bool { let Ok(size) = fs::metadata(path).map(|meta| meta.len()) else { return false }; fs::read_to_string(verification_marker_path(path)).map(|value| value.trim() == format!("{checksum}:{size}")).unwrap_or(false) }
fn verify_model_file(path: &std::path::Path, expected: &str) -> Result<bool, String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?; let mut hash = Sha256::new(); let mut buffer = [0u8; 262_144];
    loop { let read = file.read(&mut buffer).map_err(|e| e.to_string())?; if read == 0 { break; } hash.update(&buffer[..read]); }
    let digest = hash.finalize();
    let actual = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    Ok(actual == expected)
}
fn model_is_installed(directory: &std::path::Path, model: &ModelSpec) -> bool { fs::metadata(directory.join(model.file)).map(|meta| meta.len() >= model.min_bytes).unwrap_or(false) }
fn active_model_id(app: &AppHandle) -> Option<String> { fs::read_to_string(app.path().app_local_data_dir().ok()?.join(ACTIVE_MODEL_FILE)).ok().map(|value| value.trim().to_string()) }
fn model_list(app: &AppHandle) -> Result<Vec<TextModel>, String> {
    let directory = text_models_dir(app)?; let active = active_model_id(app);
    Ok(TEXT_MODELS.iter().map(|model| { let installed_bytes = fs::metadata(directory.join(model.file)).map(|meta| meta.len()).unwrap_or(0); let partial_bytes = fs::metadata(part_path(&directory.join(model.file))).map(|meta| meta.len()).unwrap_or(0); TextModel { id: model.id.into(), name: model.name.into(), description: model.description.into(), size_mb: model.size_mb, installed: installed_bytes >= model.min_bytes, active: active.as_deref() == Some(model.id), installed_bytes, partial_bytes } }).collect())
}
fn part_path(target: &std::path::Path) -> PathBuf {
    let mut name = target.file_name().map(|value| value.to_string_lossy().to_string()).unwrap_or_default(); name.push_str(".part"); target.with_file_name(name)
}
fn is_bundled(model: &ModelSpec) -> bool { model.url.is_empty() }
fn whisper_models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app.path().app_local_data_dir().map_err(|e| e.to_string())?.join("whisper-models");
    fs::create_dir_all(&directory).map_err(|e| format!("Transkriptionsordner konnte nicht erstellt werden: {e}"))?; Ok(directory)
}
fn whisper_spec(id: &str) -> Result<&'static ModelSpec, String> { WHISPER_MODELS.iter().find(|model| model.id == id).ok_or_else(|| "Unbekanntes Transkriptionsmodell.".into()) }
fn active_whisper_id(app: &AppHandle) -> String {
    app.path().app_local_data_dir().ok().and_then(|directory| fs::read_to_string(directory.join(ACTIVE_WHISPER_FILE)).ok()).map(|value| value.trim().to_string()).filter(|value| whisper_spec(value).is_ok()).unwrap_or_else(|| BUNDLED_WHISPER_ID.to_string())
}
fn whisper_file_installed(directory: &std::path::Path, model: &ModelSpec) -> bool { fs::metadata(directory.join(model.file)).map(|meta| meta.len() >= model.min_bytes).unwrap_or(false) }
fn active_whisper_model_path(app: &AppHandle) -> Result<PathBuf, String> {
    let id = active_whisper_id(app);
    if let Ok(model) = whisper_spec(&id) {
        if !is_bundled(model) {
            let directory = whisper_models_dir(app)?;
            if whisper_file_installed(&directory, model) { return Ok(directory.join(model.file)); }
        }
    }
    bundled_model_path(app)
}
fn whisper_model_list(app: &AppHandle) -> Result<Vec<TextModel>, String> {
    let directory = whisper_models_dir(app)?; let active = active_whisper_id(app);
    Ok(WHISPER_MODELS.iter().map(|model| {
        let bundled = is_bundled(model);
        let path = if bundled { bundled_model_path(app).unwrap_or_default() } else { directory.join(model.file) };
        let installed_bytes = fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
        let partial_bytes = if bundled { 0 } else { fs::metadata(part_path(&directory.join(model.file))).map(|meta| meta.len()).unwrap_or(0) };
        TextModel { id: model.id.into(), name: model.name.into(), description: model.description.into(), size_mb: model.size_mb, installed: installed_bytes >= model.min_bytes, active: active == model.id, installed_bytes, partial_bytes }
    }).collect())
}
fn on_wifi() -> bool {
    #[cfg(target_os = "macos")]
    {
        let route = match Command::new("/sbin/route").args(["-n", "get", "default"]).output() { Ok(output) => output, Err(_) => return false };
        let device = match String::from_utf8_lossy(&route.stdout).lines().find_map(|line| line.trim().strip_prefix("interface:").map(|value| value.trim().to_string())) { Some(device) => device, None => return false };
        let order = match Command::new("/usr/sbin/networksetup").arg("-listnetworkserviceorder").output() { Ok(output) => output, Err(_) => return false };
        String::from_utf8_lossy(&order.stdout).lines().filter(|line| line.contains(&format!("Device: {device})"))).any(|line| line.contains("Wi-Fi") || line.contains("AirPort"))
    }
    #[cfg(not(target_os = "macos"))]
    { true }
}
fn download_model_file(app: &AppHandle, downloads: &DownloadState, model: &ModelSpec, directory: &std::path::Path, wifi_only: bool) -> Result<(), String> {
    if is_bundled(model) { return Ok(()); }
    let target = directory.join(model.file); let temporary = part_path(&target); let model_id = model.id.to_string();
    downloads.0.lock().map_err(|_| "Downloadstatus konnte nicht gesperrt werden.")?.remove(&model_id);
    if fs::metadata(&target).map(|meta| meta.len() >= model.min_bytes).unwrap_or(false) { return Ok(()); }
    let existing = fs::metadata(&temporary).map(|meta| meta.len()).unwrap_or(0);
    if existing >= model.min_bytes { fs::rename(&temporary, &target).map_err(|e| e.to_string())?; return Ok(()); }
    if wifi_only && !on_wifi() { return Err("Große Modelldownloads sind auf WLAN beschränkt. Verbinde dich mit einem WLAN oder deaktiviere die Einstellung.".into()); }
    let client = reqwest::blocking::Client::new(); let mut request = client.get(model.url);
    if existing > 0 { request = request.header(reqwest::header::RANGE, format!("bytes={existing}-")); }
    let mut response = request.send().map_err(|e| format!("Modelldownload konnte nicht gestartet werden: {e}"))?.error_for_status().map_err(|e| format!("Modelldownload fehlgeschlagen: {e}"))?;
    let resumed = existing > 0 && response.status() == reqwest::StatusCode::PARTIAL_CONTENT; let downloaded_start = if resumed { existing } else { 0 };
    let total = response.content_length().map(|length| length + downloaded_start);
    let mut file = if resumed { fs::OpenOptions::new().append(true).open(&temporary) } else { fs::File::create(&temporary) }.map_err(|e| format!("Modell konnte nicht gespeichert werden: {e}"))?;
    let mut downloaded = downloaded_start; let mut last_emitted = 0;
    app.emit("model-download-progress", DownloadProgress { model_id: model_id.clone(), downloaded_bytes: downloaded, total_bytes: total }).map_err(|e| e.to_string())?;
    let mut buffer = [0u8; 262_144];
    loop {
        if downloads.0.lock().map_err(|_| "Downloadstatus konnte nicht gesperrt werden.")?.contains(&model_id) { return Err("Modelldownload abgebrochen. Der bisherige Fortschritt bleibt zum Fortsetzen erhalten.".into()); }
        let read = response.read(&mut buffer).map_err(|e| format!("Modelldownload wurde unterbrochen: {e}"))?; if read == 0 { break; }
        file.write_all(&buffer[..read]).map_err(|e| format!("Modell konnte nicht gespeichert werden: {e}"))?; downloaded += read as u64;
        if downloaded - last_emitted >= 1_048_576 { last_emitted = downloaded; app.emit("model-download-progress", DownloadProgress { model_id: model_id.clone(), downloaded_bytes: downloaded, total_bytes: total }).map_err(|e| e.to_string())?; }
    }
    app.emit("model-download-progress", DownloadProgress { model_id: model_id.clone(), downloaded_bytes: downloaded, total_bytes: total }).map_err(|e| e.to_string())?;
    if downloaded < model.min_bytes { return Err("Der Modelldownload ist unvollständig. Du kannst ihn später fortsetzen.".into()); }
    if !verify_model_file(&temporary, model.sha256)? { let _ = fs::remove_file(&temporary); return Err("Die Prüfsumme des Modells stimmt nicht. Die fehlerhafte Datei wurde entfernt.".into()); }
    fs::rename(&temporary, &target).map_err(|e| format!("Modell konnte nicht aktiviert werden: {e}"))?; write_verification_marker(&target, model.sha256)?; Ok(())
}
fn active_text_model_path(app: &AppHandle) -> Result<PathBuf, String> {
    let id = active_model_id(app).ok_or("Bitte wähle zuerst ein lokales Schreibmodell.")?;
    let model = spec(&id)?; let directory = text_models_dir(app)?;
    if !model_is_installed(&directory, model) { return Err("Das aktive Schreibmodell ist nicht vollständig installiert.".into()); }
    Ok(directory.join(model.file))
}
fn rewrite_instruction(action: &str, target_language: Option<&str>) -> Result<String, String> {
    match action {
        "shorten" => Ok("Kürze den Text deutlich. Bewahre alle Fakten, Namen, Zahlen und Entscheidungen.".into()),
        "professional" => Ok("Formuliere den Text professionell, klar und höflich. Bewahre alle Fakten.".into()),
        "email" => Ok("Forme den Text in eine kurze, freundliche E-Mail mit passender Anrede und Grußformel um.".into()),
        "bullets" => Ok("Fasse den Text als präzise Stichpunkte zusammen. Erfinde keine Informationen.".into()),
        "correct" => match target_language {
            Some("de") => Ok("Wende die zuletzt gesprochene Selbstkorrektur im Text an. Korrekturen können als 'nein, X', 'nein X', 'ich meine X', 'äh X' oder 'Korrektur: X' auftreten — mit oder ohne Komma, auch ohne das Wort 'ich meine'. Das Komma oder fehlende Komma spielt keine Rolle; entscheidend ist nur das Korrekturwort direkt nach dem falschen Begriff. Ersetze ausschließlich den unmittelbar davor genannten fehlerhaften Begriff oder Satzteil durch X und entferne den Korrekturhinweis selbst vollständig — dies ist wichtiger als reine Zeichensetzung. Beispiele: 'Mein Name ist Marco nein Daniel' wird zu 'Mein Name ist Daniel'. 'Mein Name ist Marco, nein Daniel' wird ebenfalls zu 'Mein Name ist Daniel'. Gib nur den fertigen Text aus, ausschließlich auf Deutsch, ohne Erklärung, ohne Denkschritte. Erfinde keine Informationen.".into()),
            Some("en") => Ok("Apply the latest spoken self-correction in the text. Corrections can appear as 'no, X', 'no X', 'I mean X', or 'correction: X' — even without the words 'I mean'. Replace only the immediately preceding incorrect word or phrase with X and remove the correction marker entirely. Example: 'My name is Marco, no Daniel' becomes 'My name is Daniel'. Output only the final text, exclusively in English, without explanation or reasoning. Do not invent information.".into()),
            Some("fr") => Ok("Applique la dernière autocorrection prononcée dans le texte. Les corrections peuvent apparaître comme 'non, X', 'non X', 'je veux dire X' ou 'correction : X' — même sans 'je veux dire'. Remplace uniquement le terme erroné précédent par X et supprime entièrement l'indication de correction. Exemple : 'Je m'appelle Marco, non Daniel' devient 'Je m'appelle Daniel'. Ne donne que le texte final, uniquement en français, sans explication. N'invente aucune information.".into()),
            Some("es") => Ok("Aplica la última autocorrección hablada en el texto. Las correcciones pueden aparecer como 'no, X', 'no X', 'quiero decir X' o 'corrección: X' — incluso sin 'quiero decir'. Reemplaza únicamente el término erróneo anterior por X y elimina por completo la marca de corrección. Ejemplo: 'Me llamo Marco, no Daniel' se convierte en 'Me llamo Daniel'. Devuelve solo el texto final, únicamente en español, sin explicación. No inventes información.".into()),
            _ => Err("Ungültige Quellsprache für die lokale Selbstkorrektur.".into()),
        },
        "translate" => match target_language {
            Some("en") => Ok("Übersetze den Text idiomatisch ins Englische. Bewahre Namen, Zahlen, Fachbegriffe und Formatierung.".into()),
            Some("de") => Ok("Übersetze den Text idiomatisch ins Deutsche. Bewahre Namen, Zahlen, Fachbegriffe und Formatierung.".into()),
            Some("fr") => Ok("Übersetze den Text idiomatisch ins Französische. Bewahre Namen, Zahlen, Fachbegriffe und Formatierung.".into()),
            Some("es") => Ok("Übersetze den Text idiomatisch ins Spanische. Bewahre Namen, Zahlen, Fachbegriffe und Formatierung.".into()),
            _ => Err("Ungültige Zielsprache für die lokale Übersetzung.".into()),
        },
        _ => Err("Unbekannte lokale Textaktion.".into()),
    }
}
fn llm_worker_path(app: &AppHandle) -> Result<PathBuf, String> {
    if cfg!(debug_assertions) { return std::env::current_exe().map_err(|e| e.to_string())?.parent().ok_or("Debug-Ordner konnte nicht ermittelt werden.".into()).map(|directory| directory.join("llm-worker")); }
    let path = app.path().resource_dir().map_err(|e| e.to_string())?.join("binaries/llm-worker-aarch64-apple-darwin");
    if !path.is_file() { return Err("Der integrierte Schreib-Worker fehlt. Bitte die App erneut installieren.".into()); } Ok(path)
}
fn append_diagnostic_log(app: &AppHandle, message: &str) {
    if message.trim().is_empty() { return; }
    if let Ok(cache) = app.path().app_cache_dir() { let _ = fs::create_dir_all(&cache); let path = cache.join("diagnostics.log"); if fs::metadata(&path).map(|meta| meta.len() > 1_048_576).unwrap_or(false) { let _ = fs::write(&path, "[Log rotiert: vorherige Einträge wurden aus Datenschutzgründen entfernt]\n"); } let _ = fs::OpenOptions::new().create(true).append(true).open(path).and_then(|mut file| writeln!(file, "{}\n{}", chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]"), message.trim())); }
}
#[tauri::command]
fn read_diagnostic_log(app: AppHandle) -> String { app.path().app_cache_dir().ok().and_then(|cache| fs::read_to_string(cache.join("diagnostics.log")).ok()).unwrap_or_else(|| "Noch keine lokalen Diagnoseeinträge.".into()) }
#[tauri::command]
fn log_diagnostic(app: AppHandle, message: String) { append_diagnostic_log(&app, &message); }
#[tauri::command]
fn clear_diagnostic_log(app: AppHandle) { if let Ok(cache) = app.path().app_cache_dir() { let _ = fs::remove_file(cache.join("diagnostics.log")); } }

fn dispatch_worker_line(app: &AppHandle, pending: &Mutex<HashMap<String, Sender<WorkerEvent>>>, line: &str) {
    let value: serde_json::Value = match serde_json::from_str(line) { Ok(value) => value, Err(_) => return };
    let kind = value.get("type").and_then(|v| v.as_str()).unwrap_or(""); if kind == "ready" { return; }
    let id = value.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    match kind {
        "token" => {
            let token = value.get("token").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let _ = app.emit("rewrite-token", RewriteToken { request_id: id.clone(), token: token.clone() });
            if let Ok(guard) = pending.lock() { if let Some(sender) = guard.get(&id) { let _ = sender.send(WorkerEvent::Token(token)); } }
        }
        "done" => { if let Ok(mut guard) = pending.lock() { if let Some(sender) = guard.remove(&id) { let _ = sender.send(WorkerEvent::Done); } } }
        "error" => { let message = value.get("message").and_then(|v| v.as_str()).unwrap_or("Unbekannter Fehler bei der lokalen Textaufgabe.").to_string(); if let Ok(mut guard) = pending.lock() { if let Some(sender) = guard.remove(&id) { let _ = sender.send(WorkerEvent::Error(message)); } } }
        "cancelled" => { if let Ok(mut guard) = pending.lock() { if let Some(sender) = guard.remove(&id) { let _ = sender.send(WorkerEvent::Cancelled); } } }
        _ => {}
    }
}

fn spawn_worker(app: &AppHandle, workers: &LlmWorkerState, model_path: PathBuf) -> Result<Arc<LlmWorkerHandle>, String> {
    // Hold the lock for the whole spawn (including the blocking model load) so a
    // concurrent warm-up and an explicit rewrite click cannot start two worker
    // processes for the same model at once and double the memory/CPU cost.
    let mut guard = workers.active.lock().map_err(|_| "LLM-Workerstatus konnte nicht gesperrt werden.")?;
    if let Some(handle) = guard.as_ref() {
        if handle.model_path == model_path { *handle.last_used.lock().unwrap() = Instant::now(); return Ok(handle.clone()); }
    }
    if let Some(old) = guard.take() {
        // Hard-kill only: a graceful "shutdown" message lets the worker return
        // normally from main(), which runs ggml's Metal static destructors —
        // a known crash (SIGABRT in ggml_metal_rsets_free) on process exit.
        // SIGKILL skips destructors entirely and avoids that crash reliably.
        let _ = old.child.lock().map(|mut child| child.kill());
    }
    let worker_path = llm_worker_path(app)?;
    let mut child = Command::new(&worker_path).arg(model_path.to_str().ok_or("Ungültiger Modellpfad")?).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("Integrierter Schreib-Worker konnte nicht gestartet werden: {e}"))?;
    let stdin = child.stdin.take().ok_or("LLM-Worker-Eingabe konnte nicht geöffnet werden.")?;
    let stdout = child.stdout.take().ok_or("LLM-Worker-Ausgabe konnte nicht geöffnet werden.")?;
    let stderr = child.stderr.take().ok_or("LLM-Worker-Fehlerausgabe konnte nicht geöffnet werden.")?;
    // Drain stderr concurrently from the moment the child starts. llama.cpp writes a
    // large volume of tensor/Metal diagnostics while loading; if nobody reads stderr
    // until after we see "ready" on stdout, the OS pipe buffer fills up and the child
    // blocks on write() before it ever reaches the ready print — a classic pipe deadlock.
    let stderr_buffer = Arc::new(Mutex::new(String::new()));
    { let app = app.clone(); let stderr_buffer = stderr_buffer.clone(); let mut stderr = stderr;
      thread::spawn(move || { let mut buffer = String::new(); let _ = stderr.read_to_string(&mut buffer); if let Ok(mut shared) = stderr_buffer.lock() { *shared = buffer.clone(); } append_diagnostic_log(&app, &buffer); }); }
    let mut reader = BufReader::new(stdout); let mut line = String::new();
    let read = reader.read_line(&mut line).map_err(|e| format!("Der lokale Schreib-Worker antwortete nicht: {e}"))?;
    if read == 0 || !line.contains("\"ready\"") {
        let _ = child.kill(); thread::sleep(Duration::from_millis(150));
        let error = stderr_buffer.lock().map(|value| value.clone()).unwrap_or_default();
        return Err(if error.trim().is_empty() { "Der lokale Schreib-Worker konnte nicht initialisiert werden.".into() } else { error.trim().into() });
    }
    let handle = Arc::new(LlmWorkerHandle { child: Mutex::new(child), stdin: Mutex::new(stdin), model_path, last_used: Mutex::new(Instant::now()) });
    { let app = app.clone(); let pending = workers.pending.clone(); let active = workers.active.clone(); let handle = handle.clone();
      thread::spawn(move || {
          let mut reader = reader; let mut line = String::new();
          loop { line.clear(); let read = match reader.read_line(&mut line) { Ok(read) => read, Err(_) => break }; if read == 0 { break; } let trimmed = line.trim(); if !trimmed.is_empty() { dispatch_worker_line(&app, &pending, trimmed); } }
          if let Ok(mut guard) = pending.lock() { for (_, sender) in guard.drain() { let _ = sender.send(WorkerEvent::Error("Der lokale Schreib-Worker wurde unerwartet beendet.".into())); } }
          if let Ok(mut guard) = active.lock() { if guard.as_ref().map(|current| Arc::ptr_eq(current, &handle)).unwrap_or(false) { *guard = None; } }
      });
    }
    { let active = workers.active.clone(); let handle = handle.clone();
      thread::spawn(move || loop {
          thread::sleep(Duration::from_secs(30));
          let Ok(guard) = active.lock() else { break }; let still_active = guard.as_ref().map(|current| Arc::ptr_eq(current, &handle)).unwrap_or(false); drop(guard);
          if !still_active { break; }
          if handle.last_used.lock().map(|instant| instant.elapsed()).unwrap_or_default() >= LLM_IDLE_TIMEOUT {
              if let Ok(mut guard) = active.lock() { if guard.as_ref().map(|current| Arc::ptr_eq(current, &handle)).unwrap_or(false) { *guard = None; } }
              // Hard-kill only, see comment above: avoids the ggml Metal exit-time crash.
              let _ = handle.child.lock().map(|mut child| child.kill());
              break;
          }
      });
    }
    *guard = Some(handle.clone());
    Ok(handle)
}

fn strip_thinking(output: &str) -> String {
    // Defense in depth: Qwen3 is a reasoning model that can emit a <think> block
    // before the real answer. The prompt already pre-closes an empty think block,
    // but if a model still emits one, strip it here so it never reaches the user.
    let mut text = output.to_string();
    while let Some(start) = text.find("<think>") {
        if let Some(end) = text[start..].find("</think>") { let end = start + end + "</think>".len(); text.replace_range(start..end, ""); } else { text.truncate(start); break; }
    }
    text.trim().to_string()
}
fn generate_local(app: &AppHandle, workers: &LlmWorkerState, request_id: &str, model_path: PathBuf, prompt: &str) -> Result<String, String> {
    let handle = spawn_worker(app, workers, model_path)?;
    let (sender, receiver) = std::sync::mpsc::channel();
    workers.pending.lock().map_err(|_| "LLM-Workerstatus konnte nicht gesperrt werden.")?.insert(request_id.into(), sender);
    let request = serde_json::json!({ "type": "generate", "id": request_id, "prompt": prompt, "max_tokens": 512 });
    { let mut stdin = handle.stdin.lock().map_err(|_| "LLM-Worker-Eingabe konnte nicht gesperrt werden.")?; writeln!(stdin, "{request}").map_err(|e| format!("Anfrage an den lokalen Schreib-Worker fehlgeschlagen: {e}"))?; }
    *handle.last_used.lock().map_err(|_| "LLM-Workerstatus konnte nicht gesperrt werden.")? = Instant::now();
    let mut output = String::new();
    let result: Result<(), String> = loop {
        match receiver.recv_timeout(Duration::from_secs(90)) {
            Ok(WorkerEvent::Token(token)) => output.push_str(&token),
            Ok(WorkerEvent::Done) => break Ok(()),
            Ok(WorkerEvent::Error(message)) => break Err(message),
            Ok(WorkerEvent::Cancelled) => break Err("Die lokale Umformulierung wurde abgebrochen.".into()),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break Err("Die lokale Textaktion hat nach 90 Sekunden keine Antwort geliefert und wurde beendet.".into()),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break Err("Verbindung zum lokalen Schreib-Worker wurde unterbrochen.".into()),
        }
    };
    // Always clear the sender, including cancellation, timeout, and worker failure.
    if let Ok(mut pending) = workers.pending.lock() { pending.remove(request_id); }
    result?;
    let cleaned = strip_thinking(&output); if cleaned.is_empty() { return Err("Das lokale Modell hat keine Ausgabe erzeugt.".into()); } Ok(cleaned)
}
#[tauri::command]
async fn rewrite_local(app: AppHandle, workers: State<'_, LlmWorkerState>, text: String, action: String, target_language: Option<String>, request_id: String) -> Result<String, String> {
    let instruction = rewrite_instruction(&action, target_language.as_deref())?; if text.trim().is_empty() { return Err("Es gibt keinen Text zum Umformulieren.".into()); }
    let clipped = text.chars().take(12_000).collect::<String>(); let prompt = format!("<|im_start|>system\nDu bist ein lokaler Schreibassistent. {instruction} Antworte ausschließlich mit dem fertigen Text, ohne Erklärung, ohne Markdown-Codeblöcke und ohne Denkschritte oder <think>-Tags.<|im_end|>\n<|im_start|>user\nTEXT:\n{clipped}\n/no_think<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"); let workers = workers.inner().clone(); let model = active_text_model_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || generate_local(&app, &workers, &request_id, model, &prompt)).await.map_err(|e| format!("Lokale Schreibaufgabe fehlgeschlagen: {e}"))?
}

#[tauri::command]
async fn warm_up_text_model(app: AppHandle, workers: State<'_, LlmWorkerState>) -> Result<bool, String> {
    // Called at startup and after activating a model so the Metal load happens
    // proactively instead of during the user's first rewrite click.
    let Ok(model) = active_text_model_path(&app) else { return Ok(false); };
    let workers = workers.inner().clone();
    tauri::async_runtime::spawn_blocking(move || spawn_worker(&app, &workers, model).map(|_| true)).await.map_err(|e| format!("Aufwärmen fehlgeschlagen: {e}"))?
}

#[tauri::command]
fn cancel_local_rewrite(workers: State<LlmWorkerState>, request_id: String) -> Result<(), String> {
    if let Some(handle) = workers.active.lock().map_err(|_| "LLM-Workerstatus konnte nicht gesperrt werden.")?.as_ref() {
        let request = serde_json::json!({ "type": "cancel", "id": request_id });
        let mut stdin = handle.stdin.lock().map_err(|_| "LLM-Worker-Eingabe konnte nicht gesperrt werden.")?;
        writeln!(stdin, "{request}").map_err(|e| format!("Abbruch konnte nicht gesendet werden: {e}"))?;
    }
    Ok(())
}
fn whisper_worker_path(app: &AppHandle) -> Result<PathBuf, String> {
    if cfg!(debug_assertions) {
        return std::env::current_exe().map_err(|e| e.to_string())?.parent().ok_or("Debug-Ordner konnte nicht ermittelt werden.".into()).map(|directory| directory.join("whisper-worker"));
    }
    let path = app.path().resource_dir().map_err(|e| e.to_string())?.join("binaries/whisper-worker-aarch64-apple-darwin");
    if !path.is_file() { return Err("Der integrierte Whisper-Worker fehlt. Bitte die App erneut installieren.".into()); }
    Ok(path)
}

#[tauri::command]
fn get_system_status(app: AppHandle) -> SystemStatus {
    let whisper_ready = active_whisper_model_path(&app).is_ok(); let whisper_worker_ready = whisper_worker_path(&app).map(|path| path.is_file()).unwrap_or(false); let llm_worker_ready = llm_worker_path(&app).map(|path| path.is_file()).unwrap_or(false);
    // Integrity is validated on download and explicit activation. Do not hash a
    // multi-gigabyte model during startup, because that blocks initial readiness.
    let active_model_ready = active_text_model_path(&app).is_ok();
    // Check trust status WITHOUT the prompt flag so the automatic startup status
    // check can never trigger or block on the macOS consent dialog. Requesting the
    // permission itself happens only via the explicit `request_accessibility_permission`
    // command, invoked by a deliberate user action (matches microphone handling).
    #[cfg(target_os = "macos")]
    let accessibility_granted = macos_accessibility_client::accessibility::application_is_trusted();
    #[cfg(not(target_os = "macos"))]
    let accessibility_granted = false;
    SystemStatus { whisper_ready, whisper_worker_ready, llm_worker_ready, active_model_ready, accessibility_granted }
}
#[tauri::command] fn initialize_local_engine(app: AppHandle) -> Result<EngineStatus, String> { let path = bundled_model_path(&app)?; Ok(EngineStatus { ready: true, message: format!("Integrierte Sprach-Engine bereit ({})", path.file_name().unwrap_or_default().to_string_lossy()) }) }
#[tauri::command] fn list_text_models(app: AppHandle) -> Result<Vec<TextModel>, String> { model_list(&app) }
#[tauri::command]
async fn download_text_model(app: AppHandle, downloads: State<'_, DownloadState>, model_id: String, wifi_only: Option<bool>) -> Result<Vec<TextModel>, String> {
    let state = downloads.0.clone();
    tauri::async_runtime::spawn_blocking(move || { let downloads = DownloadState(state); let model = spec(&model_id)?; let directory = text_models_dir(&app)?; download_model_file(&app, &downloads, model, &directory, wifi_only.unwrap_or(true))?; fs::write(app.path().app_local_data_dir().map_err(|e| e.to_string())?.join(ACTIVE_MODEL_FILE), model.id).map_err(|e| format!("Modellauswahl konnte nicht gespeichert werden: {e}"))?; model_list(&app) }).await.map_err(|e| format!("Modelldownload-Task fehlgeschlagen: {e}"))?
}
#[tauri::command] fn list_whisper_models(app: AppHandle) -> Result<Vec<TextModel>, String> { whisper_model_list(&app) }
#[tauri::command]
async fn download_whisper_model(app: AppHandle, downloads: State<'_, DownloadState>, model_id: String, wifi_only: Option<bool>) -> Result<Vec<TextModel>, String> {
    let state = downloads.0.clone();
    tauri::async_runtime::spawn_blocking(move || { let downloads = DownloadState(state); let model = whisper_spec(&model_id)?; let directory = whisper_models_dir(&app)?; download_model_file(&app, &downloads, model, &directory, wifi_only.unwrap_or(true))?; fs::write(app.path().app_local_data_dir().map_err(|e| e.to_string())?.join(ACTIVE_WHISPER_FILE), model.id).map_err(|e| format!("Transkriptionsauswahl konnte nicht gespeichert werden: {e}"))?; whisper_model_list(&app) }).await.map_err(|e| format!("Transkriptionsmodell-Download-Task fehlgeschlagen: {e}"))?
}
fn select_whisper_model_internal(app: AppHandle, model_id: String) -> Result<Vec<TextModel>, String> {
    let model = whisper_spec(&model_id)?;
    if is_bundled(model) { bundled_model_path(&app)?; } else {
        let directory = whisper_models_dir(&app)?;
        if !whisper_file_installed(&directory, model) { return Err("Dieses Transkriptionsmodell muss zuerst vollständig heruntergeladen werden.".into()); }
        if !verify_model_file(&directory.join(model.file), model.sha256)? { return Err("Die Prüfsumme dieses Transkriptionsmodells stimmt nicht. Bitte lösche und lade es erneut herunter.".into()); }
    }
    fs::write(app.path().app_local_data_dir().map_err(|e| e.to_string())?.join(ACTIVE_WHISPER_FILE), model.id).map_err(|e| format!("Transkriptionsauswahl konnte nicht gespeichert werden: {e}"))?;
    whisper_model_list(&app)
}
#[tauri::command]
async fn select_whisper_model(app: AppHandle, model_id: String) -> Result<Vec<TextModel>, String> {
    tauri::async_runtime::spawn_blocking(move || select_whisper_model_internal(app, model_id)).await.map_err(|e| format!("Transkriptionsmodell-Aktivierung fehlgeschlagen: {e}"))?
}

#[tauri::command]
fn delete_whisper_model(app: AppHandle, downloads: State<DownloadState>, model_id: String) -> Result<Vec<TextModel>, String> {
    let model = whisper_spec(&model_id)?;
    if is_bundled(model) { return Err("Das mitgelieferte Transkriptionsmodell kann nicht gelöscht werden.".into()); }
    if active_whisper_id(&app) == model.id { return Err("Das aktive Transkriptionsmodell kann nicht gelöscht werden. Wähle zuerst ein anderes Profil.".into()); }
    if downloads.0.lock().map_err(|_| "Downloadstatus konnte nicht gesperrt werden.")?.contains(model.id) { return Err("Ein laufender Download kann nicht gelöscht werden. Brich ihn zuerst ab.".into()); }
    let directory = whisper_models_dir(&app)?; let target = directory.join(model.file);
    let _ = fs::remove_file(&target); let _ = fs::remove_file(part_path(&target)); whisper_model_list(&app)
}
#[tauri::command]
fn cancel_text_model_download(downloads: State<DownloadState>, model_id: String) -> Result<(), String> { downloads.0.lock().map_err(|_| "Downloadstatus konnte nicht gesperrt werden.")?.insert(model_id); Ok(()) }
#[tauri::command]
fn delete_text_model(app: AppHandle, downloads: State<DownloadState>, model_id: String) -> Result<Vec<TextModel>, String> {
    let model = spec(&model_id)?; let directory = text_models_dir(&app)?;
    if active_model_id(&app).as_deref() == Some(model.id) { return Err("Das aktive Modell kann nicht gelöscht werden. Aktiviere zuerst ein anderes Modell.".into()); }
    if downloads.0.lock().map_err(|_| "Downloadstatus konnte nicht gesperrt werden.")?.contains(model.id) { return Err("Ein laufender Download kann nicht gelöscht werden. Brich ihn zuerst ab.".into()); }
    let target = directory.join(model.file); let _ = fs::remove_file(&target); let _ = fs::remove_file(part_path(&target)); let _ = fs::remove_file(verification_marker_path(&target)); model_list(&app)
}
fn select_text_model_internal(app: AppHandle, model_id: String) -> Result<Vec<TextModel>, String> {
    let emit_status = |phase: &'static str, message: &str| { let _ = app.emit("model-activation-status", ActivationStatus { model_id: model_id.clone(), phase, message: message.into() }); };
    let result = (|| {
        let model = spec(&model_id)?; let directory = text_models_dir(&app)?;
        if !model_is_installed(&directory, model) { return Err("Dieses Modell muss zuerst vollständig heruntergeladen werden.".into()); }
        let path = directory.join(model.file);
        if has_verification_marker(&path, model.sha256) {
            emit_status("activating", "Prüfsumme bereits bestätigt — Modell wird aktiviert …");
        } else {
            emit_status("verifying", "Prüfsumme wird berechnet (einmalig pro Modell) …");
            if !verify_model_file(&path, model.sha256)? { return Err("Die Prüfsumme dieses Modells stimmt nicht. Bitte lösche und lade es erneut herunter.".into()); }
            write_verification_marker(&path, model.sha256)?;
            emit_status("activating", "Prüfsumme bestätigt — Modell wird aktiviert …");
        }
        fs::write(app.path().app_local_data_dir().map_err(|e| e.to_string())?.join(ACTIVE_MODEL_FILE), model.id).map_err(|e| format!("Modellauswahl konnte nicht gespeichert werden: {e}"))?;
        model_list(&app)
    })();
    match &result { Ok(_) => emit_status("activated", "Modell ist aktiv."), Err(message) => emit_status("error", message) }
    result
}
#[tauri::command]
async fn select_text_model(app: AppHandle, model_id: String) -> Result<Vec<TextModel>, String> {
    tauri::async_runtime::spawn_blocking(move || select_text_model_internal(app, model_id)).await.map_err(|e| format!("Modellaktivierung fehlgeschlagen: {e}"))?
}

fn active_window_overlay_position() -> Option<(f64, f64)> {
    #[cfg(target_os = "macos")]
    {
        // Accessibility coordinates are global across all displays, so the overlay
        // follows the user's active window even on a secondary monitor.
        let script = r#"tell application "System Events"
            set focusedProcess to first process whose frontmost is true
            set focusedWindow to window 1 of focusedProcess
            set {xPos, yPos} to position of focusedWindow
            set {windowWidth, windowHeight} to size of focusedWindow
            return (xPos as text) & "," & (yPos as text) & "," & (windowWidth as text) & "," & (windowHeight as text)
        end tell"#;
        let output = Command::new("osascript").args(["-e", script]).output().ok()?;
        if !output.status.success() { return None; }
        let values: Vec<f64> = String::from_utf8_lossy(&output.stdout).trim().split(',').filter_map(|value| value.trim().parse().ok()).collect();
        if values.len() != 4 { return None; }
        // Keep the 370×96 overlay centered at the bottom of the focused window.
        Some((values[0] + (values[2] - 370.0) / 2.0, values[1] + values[3] - 116.0))
    }
    #[cfg(not(target_os = "macos"))]
    { None }
}

#[tauri::command]
fn show_dictation_overlay(app: AppHandle) -> Result<(), String> {
    let overlay = app.get_webview_window("dictation").ok_or("Diktat-Leiste wurde nicht initialisiert.")?;
    // The overlay must always become visible, even when Voicely's main window is
    // deliberately hidden after auto-paste. Start from a guaranteed on-screen
    // location, then move to the focused window only if its coordinates are sane.
    let _ = overlay.center();
    overlay.show().map_err(|e| format!("Diktat-Leiste konnte nicht eingeblendet werden: {e}"))?;
    if let Some((x, y)) = active_window_overlay_position().filter(|(x, y)| x.is_finite() && y.is_finite() && *x > -10_000.0 && *x < 10_000.0 && *y > -10_000.0 && *y < 10_000.0) {
        if overlay.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y))).is_err() { let _ = overlay.center(); }
    }
    Ok(())
}
#[tauri::command]
fn hide_dictation_overlay(app: AppHandle) -> Result<(), String> {
    let overlay = app.get_webview_window("dictation").ok_or("Diktat-Leiste wurde nicht initialisiert.")?;
    overlay.hide().map_err(|e| format!("Diktat-Leiste konnte nicht ausgeblendet werden: {e}"))?;
    Ok(())
}
#[tauri::command]
async fn request_accessibility_permission() -> bool {
    // Showing the system consent dialog can block for a while on the calling
    // thread, so run it off the async runtime's main thread like other
    // permission-sensitive or slow native calls in this app.
    #[cfg(target_os = "macos")]
    { tauri::async_runtime::spawn_blocking(macos_accessibility_client::accessibility::application_is_trusted_with_prompt).await.unwrap_or(false) }
    #[cfg(not(target_os = "macos"))]
    { false }
}

#[derive(Deserialize)] #[serde(rename_all = "camelCase")]
struct CustomAppCommand { phrase: String, app_path: String }

#[tauri::command]
fn execute_voice_command(command: String, custom_apps: Vec<CustomAppCommand>) -> Result<String, String> {
    let normalized = command.trim().to_lowercase();
    let app = match normalized.as_str() {
        "öffne meine e-mails" | "öffne meine emails" | "öffne mail" => Some("Mail".to_string()),
        "öffne kalender" | "öffne meinen kalender" => Some("Calendar".to_string()),
        "öffne iterm" | "öffne terminal" | "öffne mein terminal" => Some("iTerm".to_string()),
        "öffne notizen" | "öffne meine notizen" => Some("Notes".to_string()),
        _ => custom_apps.iter().find(|entry| normalized == entry.phrase.trim().to_lowercase()).map(|entry| entry.app_path.clone()),
    }.ok_or_else(|| "Befehl nicht erkannt. Verfügbar sind Mail, Kalender, iTerm, Notizen und deine eigenen App-Befehle.".to_string())?;
    let is_custom = custom_apps.iter().any(|entry| entry.app_path == app);
    if is_custom && (!app.ends_with(".app") || !std::path::Path::new(&app).is_dir()) { return Err("Der konfigurierte App-Befehl verweist nicht auf ein lokales .app-Bundle.".into()); }
    let status = if is_custom { Command::new("open").arg(&app).status() } else { Command::new("open").args(["-a", &app]).status() }.map_err(|error| format!("App konnte nicht geöffnet werden: {error}"))?;
    if !status.success() { return Err("Die gewünschte lokale App konnte nicht geöffnet werden.".into()); }
    Ok(format!("{} wird geöffnet", if is_custom { command } else { app }))
}

#[tauri::command]
fn frontmost_application_name() -> Option<String> {
    #[cfg(target_os = "macos")]
    { Command::new("osascript").args(["-e", r#"tell application "System Events" to get name of first process whose frontmost is true"#]).output().ok().and_then(|output| if output.status.success() { Some(String::from_utf8_lossy(&output.stdout).trim().to_string()) } else { None }).filter(|name| !name.is_empty()) }
    #[cfg(not(target_os = "macos"))]
    { None }
}
#[tauri::command]
fn paste_into_active_app(app: AppHandle, restore_window: Option<bool>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let window = app.get_webview_window("main").ok_or("App-Fenster wurde nicht gefunden.")?;
        let manual_paste = restore_window.unwrap_or(true);
        // Global dictation never focuses the main window, so automatic paste already
        // targets the user's active app. Do not hide Voicely in that path: a hidden
        // window makes the running app look like it crashed. Manual paste still hides
        // briefly to hand focus back to the previous app, then restores the window.
        if manual_paste { window.hide().map_err(|e| format!("Voicely konnte nicht ausgeblendet werden: {e}"))?; std::thread::sleep(std::time::Duration::from_millis(250)); }
        let result = std::process::Command::new("osascript").args(["-e", r#"tell application "System Events" to keystroke "v" using command down"#]).output();
        let result = result.map_err(|e| { if manual_paste { let _ = window.show(); } format!("Auto-Paste konnte nicht gestartet werden: {e}") })?;
        if !result.status.success() { if manual_paste { let _ = window.show(); } return Err("Auto-Paste wurde von macOS blockiert. Aktiviere Voicely unter Systemeinstellungen > Datenschutz & Sicherheit > Bedienungshilfen.".into()); }
        if manual_paste { let _ = window.show(); }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    { let _ = app; Err("Auto-Paste ist derzeit nur für macOS verfügbar.".into()) }
}
#[tauri::command]
fn transcribe_local(app: AppHandle, request: TranscriptionRequest) -> Result<String, String> {
    let audio = STANDARD.decode(request.wav_base64).map_err(|_| "Ungültige Audiodaten")?;
    let cache = app.path().app_cache_dir().map_err(|e| e.to_string())?; fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let input = cache.join(format!("dictation-{}.wav", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_millis()));
    fs::write(&input, audio).map_err(|e| format!("Audio konnte nicht an Whisper übergeben werden: {e}"))?;
    let result = std::process::Command::new(whisper_worker_path(&app)?).args([active_whisper_model_path(&app)?.to_str().ok_or("Ungültiger Modellpfad")?, input.to_str().ok_or("Ungültiger Audiopfad")?, request.language.as_deref().unwrap_or("auto")]).output().map_err(|e| format!("Integrierter Whisper-Worker konnte nicht gestartet werden: {e}"))?;
    let _ = fs::remove_file(&input);
    if !result.status.success() { return Err(String::from_utf8_lossy(&result.stderr).trim().to_string()); }
    let text = String::from_utf8(result.stdout).map_err(|_| "Whisper lieferte ungültigen Text.")?;
    if text.trim().is_empty() { return Err("Whisper hat keinen Text erkannt.".into()); } Ok(text.trim().into())
}

fn migrate_legacy_macos_data() {
    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        let root = PathBuf::from(home).join("Library/Application Support");
        let legacy = root.join("com.example.voicelylocal"); let current = root.join("com.voicely.local");
        // Rename is atomic on the same volume and preserves downloaded models.
        if legacy.is_dir() && !current.exists() { let _ = fs::rename(legacy, current); }
    }
}

pub fn run() {
    migrate_legacy_macos_data();
    tauri::Builder::default().manage(DownloadState::default()).manage(LlmWorkerState::default()).setup(|app| {
        WebviewWindowBuilder::new(app, "dictation", WebviewUrl::App("index.html?overlay=1".into()))
            .title("Voicely Diktat").inner_size(370.0, 96.0).center().decorations(false)
            .always_on_top(true).skip_taskbar(true).resizable(false).visible(false).build()?;
        Ok(())
    }).plugin(tauri_plugin_clipboard_manager::init()).plugin(tauri_plugin_global_shortcut::Builder::new().build()).plugin(tauri_plugin_opener::init())
      .invoke_handler(tauri::generate_handler![initialize_local_engine, get_system_status, request_accessibility_permission, read_diagnostic_log, log_diagnostic, clear_diagnostic_log, list_text_models, download_text_model, cancel_text_model_download, delete_text_model, select_text_model, list_whisper_models, download_whisper_model, select_whisper_model, delete_whisper_model, rewrite_local, cancel_local_rewrite, warm_up_text_model, show_dictation_overlay, hide_dictation_overlay, execute_voice_command, frontmost_application_name, paste_into_active_app, transcribe_local])
      .run(tauri::generate_context!()).expect("Fehler beim Starten von Voicely Local");
}
