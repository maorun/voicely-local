export type AudioProcessing = { noiseSuppression: boolean; echoCancellation: boolean; autoGainControl: boolean };
export type Recording = { finish: () => Promise<{ wavBase64: string; sampleCount: number }>; discard: () => Promise<void> };

export async function recordWav(onState: (active: boolean) => void, onLevel: (level: number) => void = () => undefined, deviceId = "default", processing: AudioProcessing = { noiseSuppression: true, echoCancellation: true, autoGainControl: true }): Promise<Recording> {
  const audio: MediaTrackConstraints = { noiseSuppression: processing.noiseSuppression, echoCancellation: processing.echoCancellation, autoGainControl: processing.autoGainControl };
  if (deviceId !== "default") audio.deviceId = { exact: deviceId };
  let stream: MediaStream;
  try { stream = await navigator.mediaDevices.getUserMedia({ audio }); }
  catch (error) { if (deviceId === "default") throw error; stream = await navigator.mediaDevices.getUserMedia({ audio: { noiseSuppression: processing.noiseSuppression, echoCancellation: processing.echoCancellation, autoGainControl: processing.autoGainControl } }); }
  const context = new AudioContext({ sampleRate: 16_000 }); const source = context.createMediaStreamSource(stream); const processor = context.createScriptProcessor(4096, 1, 1);
  const chunks: Float32Array[] = []; let smoothedLevel = 0; let released = false;
  processor.onaudioprocess = (event) => { const samples = new Float32Array(event.inputBuffer.getChannelData(0)); chunks.push(samples); let energy = 0; for (const sample of samples) energy += sample * sample; const normalized = Math.min(1, Math.max(0, (Math.sqrt(energy / samples.length) - 0.008) * 10)); smoothedLevel = smoothedLevel * 0.72 + normalized * 0.28; onLevel(smoothedLevel); };
  source.connect(processor); processor.connect(context.destination); onState(true);
  const release = async () => { if (released) return; released = true; source.disconnect(); processor.disconnect(); stream.getTracks().forEach((track) => track.stop()); await context.close(); onState(false); onLevel(0); };
  return { finish: async () => { await release(); const size = chunks.reduce((sum, chunk) => sum + chunk.length, 0); const samples = new Float32Array(size); let offset = 0; for (const chunk of chunks) { samples.set(chunk, offset); offset += chunk.length; } return { wavBase64: wavToBase64(samples, 16_000), sampleCount: samples.length }; }, discard: async () => { chunks.length = 0; await release(); } };
}

function wavToBase64(samples: Float32Array, sampleRate: number) {
  const buffer = new ArrayBuffer(44 + samples.length * 2); const view = new DataView(buffer); const write = (at: number, text: string) => [...text].forEach((c, i) => view.setUint8(at + i, c.charCodeAt(0)));
  write(0, "RIFF"); view.setUint32(4, 36 + samples.length * 2, true); write(8, "WAVE"); write(12, "fmt "); view.setUint32(16, 16, true); view.setUint16(20, 1, true); view.setUint16(22, 1, true); view.setUint32(24, sampleRate, true); view.setUint32(28, sampleRate * 2, true); view.setUint16(32, 2, true); view.setUint16(34, 16, true); write(36, "data"); view.setUint32(40, samples.length * 2, true);
  samples.forEach((sample, i) => view.setInt16(44 + i * 2, Math.max(-1, Math.min(1, sample)) * 0x7fff, true)); const bytes = new Uint8Array(buffer); let binary = ""; for (const byte of bytes) binary += String.fromCharCode(byte); return btoa(binary);
}
