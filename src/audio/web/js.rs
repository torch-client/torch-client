use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(inline_js = r#"
const RATE = 48000;
const PRESKIP = 312;
const CACHE_BYTES = 16 * 1024 * 1024;
const buffers = new Map();
const pending = new Map();
let cached = 0;
const voices = new Map();
const ended = [];
let ctx = null;

function context() {
  if (ctx) return ctx;
  const Ctor = globalThis.AudioContext || globalThis.webkitAudioContext;
  if (!Ctor) return null;
  ctx = new Ctor();
  const resume = () => {
    if (ctx.state !== "running") ctx.resume().catch(() => {});
  };
  for (const type of ["pointerdown", "keydown", "touchend"]) {
    addEventListener(type, resume, { capture: true });
  }
  return ctx;
}

function isCompacted(bytes) {
  return bytes.length >= 4 && bytes[0] === 84 && bytes[1] === 79 && bytes[2] === 80 && bytes[3] === 49;
}

async function decodeCompacted(c, bytes) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let at = 4;
  const frames = view.getUint32(at, true); at += 4;
  const rate = view.getUint32(at, true); at += 4;
  const preskip = view.getUint16(at, true); at += 2;
  const count = view.getUint32(at, true); at += 4;
  const out = new Float32Array(frames + preskip + rate);
  let written = 0;
  let failed = null;
  const decoder = new AudioDecoder({
    output: (data) => {
      const n = Math.min(data.numberOfFrames, out.length - written);
      if (n > 0) {
        data.copyTo(out.subarray(written, written + n), { planeIndex: 0, format: "f32-planar", frameCount: n });
        written += n;
      }
      data.close();
    },
    error: (e) => { failed = e; },
  });
  decoder.configure({ codec: "opus", sampleRate: rate, numberOfChannels: 1 });
  for (let i = 0; i < count; i++) {
    const length = view.getUint32(at, true); at += 4;
    decoder.decode(new EncodedAudioChunk({ type: "key", timestamp: i * 20000, data: bytes.subarray(at, at + length) }));
    at += length;
  }
  await decoder.flush();
  decoder.close();
  if (failed) throw failed;
  const skip = Math.min(preskip, written);
  const length = Math.max(1, Math.min(frames, written - skip));
  const buffer = c.createBuffer(1, length, rate);
  buffer.copyToChannel(out.subarray(skip, skip + length), 0);
  return buffer;
}

function decode(c, bytes) {
  if (isCompacted(bytes)) return decodeCompacted(c, bytes);
  return c.decodeAudioData(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));
}

export function mc_audio_known(key) {
  return buffers.has(key) || pending.has(key);
}

export function mc_audio_load(key, view) {
  const c = context();
  if (!c) return false;
  const bytes = view.slice();
  const job = decode(c, bytes).then(
    (buffer) => buffer,
    (e) => {
      console.warn(`audio: ${key} will not decode`, e);
      return null;
    },
  ).then((buffer) => {
    pending.delete(key);
    const size = buffer ? buffer.length * buffer.numberOfChannels * 4 : 0;
    if (cached + size > CACHE_BYTES) {
      buffers.clear();
      cached = 0;
    }
    buffers.set(key, buffer);
    cached += size;
    return buffer;
  });
  pending.set(key, job);
  return true;
}

export function mc_audio_forget() {
  buffers.clear();
  pending.clear();
  cached = 0;
}

function finish(id) {
  const voice = voices.get(id);
  if (!voice) return;
  voices.delete(id);
  voice.stopped = true;
  try {
    voice.gain.disconnect();
    if (voice.pan) voice.pan.disconnect();
  } catch {}
  ended.push(id);
}

export function mc_audio_start(id, key, gain, rate, pan, placed, looped) {
  const c = context();
  if (!c) return false;
  const voice = { source: null, gain: c.createGain(), pan: null, rate, stopped: false };
  voice.gain.gain.value = gain;
  if (placed && c.createStereoPanner) {
    voice.pan = c.createStereoPanner();
    voice.pan.pan.value = pan;
    voice.gain.connect(voice.pan);
    voice.pan.connect(c.destination);
  } else {
    voice.gain.connect(c.destination);
  }
  voices.set(id, voice);
  const begin = (buffer) => {
    if (voice.stopped) return;
    if (!buffer) {
      finish(id);
      return;
    }
    const source = c.createBufferSource();
    source.buffer = buffer;
    source.loop = looped;
    source.playbackRate.value = voice.rate;
    source.connect(voice.gain);
    source.onended = () => finish(id);
    source.start();
    voice.source = source;
  };
  const ready = buffers.get(key);
  if (ready !== undefined) {
    begin(ready);
  } else if (pending.has(key)) {
    pending.get(key).then(begin);
  } else {
    begin(null);
  }
  return true;
}

export function mc_audio_set(id, gain, rate, pan) {
  const voice = voices.get(id);
  if (!voice) return;
  voice.gain.gain.value = gain;
  voice.rate = rate;
  if (voice.source) voice.source.playbackRate.value = rate;
  if (voice.pan) voice.pan.pan.value = pan;
}

export function mc_audio_stop(id) {
  const voice = voices.get(id);
  if (!voice) return;
  voices.delete(id);
  voice.stopped = true;
  try {
    if (voice.source) voice.source.stop();
    voice.gain.disconnect();
    if (voice.pan) voice.pan.disconnect();
  } catch {}
}

export function mc_audio_ended() {
  return ended.splice(0);
}

let support = null;
function encoderSupported(bitrate) {
  if (support === null) {
    support = globalThis.AudioEncoder && globalThis.AudioDecoder && globalThis.OfflineAudioContext
      ? AudioEncoder.isConfigSupported({ codec: "opus", sampleRate: RATE, numberOfChannels: 1, bitrate })
          .then((r) => !!r.supported, () => false)
      : Promise.resolve(false);
  }
  return support;
}

function opusPreskip(description) {
  if (!description) return PRESKIP;
  const bytes = ArrayBuffer.isView(description)
    ? new Uint8Array(description.buffer, description.byteOffset, description.byteLength)
    : new Uint8Array(description);
  const magic = "OpusHead";
  if (bytes.length < 12 || [...magic].some((ch, i) => bytes[i] !== ch.charCodeAt(0))) return PRESKIP;
  return bytes[10] | (bytes[11] << 8);
}

export async function mc_audio_compact(view, bitrate) {
  const original = view.slice();
  if (!(await encoderSupported(bitrate))) return null;
  const decoded = await new OfflineAudioContext(1, 1, RATE).decodeAudioData(original.buffer.slice(0));
  const frames = decoded.length;
  const mono = new Float32Array(frames);
  for (let ch = 0; ch < decoded.numberOfChannels; ch++) {
    const channel = decoded.getChannelData(ch);
    for (let i = 0; i < frames; i++) mono[i] += channel[i];
  }
  if (decoded.numberOfChannels > 1) {
    const scale = 1 / decoded.numberOfChannels;
    for (let i = 0; i < frames; i++) mono[i] *= scale;
  }

  const packets = [];
  let description = null;
  let failed = null;
  const encoder = new AudioEncoder({
    output: (chunk, meta) => {
      const bytes = new Uint8Array(chunk.byteLength);
      chunk.copyTo(bytes);
      packets.push(bytes);
      if (!description && meta && meta.decoderConfig && meta.decoderConfig.description) {
        description = meta.decoderConfig.description;
      }
    },
    error: (e) => { failed = e; },
  });
  encoder.configure({ codec: "opus", sampleRate: RATE, numberOfChannels: 1, bitrate });
  for (let at = 0; at < frames; at += RATE) {
    const n = Math.min(RATE, frames - at);
    const data = new AudioData({
      format: "f32-planar",
      sampleRate: RATE,
      numberOfFrames: n,
      numberOfChannels: 1,
      timestamp: Math.round((at * 1e6) / RATE),
      data: mono.slice(at, at + n),
    });
    encoder.encode(data);
    data.close();
  }
  await encoder.flush();
  encoder.close();
  if (failed) throw failed;

  const size = 18 + packets.reduce((sum, p) => sum + 4 + p.length, 0);
  if (size >= original.length) return null;
  const out = new Uint8Array(size);
  const head = new DataView(out.buffer);
  out.set([84, 79, 80, 49], 0);
  head.setUint32(4, frames, true);
  head.setUint32(8, RATE, true);
  head.setUint16(12, opusPreskip(description), true);
  head.setUint32(14, packets.length, true);
  let at = 18;
  for (const packet of packets) {
    head.setUint32(at, packet.length, true);
    out.set(packet, at + 4);
    at += 4 + packet.length;
  }
  return out;
}
"#)]
unsafe extern "C" {
    pub(super) fn mc_audio_known(key: &str) -> bool;
    pub(super) fn mc_audio_load(key: &str, bytes: &[u8]) -> bool;
    pub(super) fn mc_audio_forget();
    pub(super) fn mc_audio_start(
        id: f64,
        key: &str,
        gain: f32,
        rate: f32,
        pan: f32,
        placed: bool,
        looped: bool,
    ) -> bool;
    pub(super) fn mc_audio_set(id: f64, gain: f32, rate: f32, pan: f32);
    pub(super) fn mc_audio_stop(id: f64);
    pub(super) fn mc_audio_ended() -> js_sys::Array;
    pub(super) fn mc_audio_compact(bytes: &[u8], bitrate: u32) -> js_sys::Promise;
}
