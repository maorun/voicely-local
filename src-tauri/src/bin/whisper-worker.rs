use std::{env, fs};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

fn wav_to_samples(data: &[u8]) -> Result<Vec<f32>, String> {
    if data.len() < 44 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" { return Err("Ungültiges WAV-Audioformat.".into()); }
    let mut index = 12; let mut format = None; let mut samples = None;
    while index + 8 <= data.len() {
        let id = &data[index..index + 4]; let length = u32::from_le_bytes(data[index + 4..index + 8].try_into().unwrap()) as usize; index += 8;
        if index + length > data.len() { return Err("Beschädigte WAV-Datei.".into()); }
        if id == b"fmt " && length >= 16 { format = Some((u16::from_le_bytes(data[index..index+2].try_into().unwrap()), u16::from_le_bytes(data[index+2..index+4].try_into().unwrap()), u16::from_le_bytes(data[index+14..index+16].try_into().unwrap()))); }
        if id == b"data" { samples = Some(&data[index..index + length]); } index += length + length % 2;
    }
    let (encoding, channels, bit_depth) = format.ok_or("WAV-Formatinformationen fehlen.")?;
    if encoding != 1 || channels != 1 || bit_depth != 16 { return Err("Erwartet wird Mono-PCM mit 16 Bit.".into()); }
    Ok(samples.ok_or("WAV-Audiodaten fehlen.")?.chunks_exact(2).map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / i16::MAX as f32).collect())
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let model = args.next().ok_or("Modellpfad fehlt.")?; let input = args.next().ok_or("Audiopfad fehlt.")?; let language = args.next().unwrap_or_else(|| "auto".into());
    let audio = fs::read(input).map_err(|e| format!("Audio konnte nicht gelesen werden: {e}"))?;
    let context = WhisperContext::new_with_params(&model, WhisperContextParameters::default()).map_err(|e| e.to_string())?;
    let mut state = context.create_state().map_err(|e| e.to_string())?;
    let mut parameters = FullParams::new(SamplingStrategy::Greedy { best_of: 1 }); parameters.set_n_threads(4); parameters.set_translate(false); parameters.set_temperature(0.0); parameters.set_suppress_blank(true); parameters.set_suppress_non_speech_tokens(true);
    if language != "auto" { parameters.set_language(Some(&language)); }
    let samples = wav_to_samples(&audio)?;
    if samples.is_empty() { return Err("Die Aufnahme enthielt keine Audiodaten. Bitte sprich etwas länger.".into()); }
    state.full(parameters, &samples).map_err(|e| format!("Lokale Transkription fehlgeschlagen: {e}"))?;
    let text = (0..state.full_n_segments().map_err(|e| e.to_string())?).filter_map(|index| state.full_get_segment_text(index).ok()).collect::<Vec<_>>().join("");
    if text.trim().is_empty() { return Err("Whisper hat keinen Text erkannt.".into()); }
    print!("{}", text.trim()); Ok(())
}
