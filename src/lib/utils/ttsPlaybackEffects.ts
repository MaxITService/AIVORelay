export type TtsPlaybackEffect = "none" | "radio" | "retro";

export type PreparedTtsPlaybackSource = {
  url: string;
  objectUrl: string | null;
  pitchCompensation: number;
};

const MAX_PROCESSING_INPUT_BYTES = 64 * 1024 * 1024;
const MAX_RENDERED_SAMPLE_VALUES = 24_000_000;

const clampPitch = (pitch: number) =>
  Number.isFinite(pitch) ? Math.min(2, Math.max(0.5, pitch)) : 1;

export async function prepareTtsPlaybackSource(
  sourceUrl: string,
  pitch: number,
  effect: TtsPlaybackEffect,
  signal: AbortSignal,
): Promise<PreparedTtsPlaybackSource> {
  const normalizedPitch = clampPitch(pitch);
  if (normalizedPitch === 1 && effect === "none") {
    return { url: sourceUrl, objectUrl: null, pitchCompensation: 1 };
  }

  const response = await fetch(sourceUrl, { signal });
  if (!response.ok) {
    throw new Error(
      `Unable to load audio for playback processing (${response.status})`,
    );
  }
  const declaredLength = Number(response.headers.get("content-length"));
  if (
    Number.isFinite(declaredLength) &&
    declaredLength > MAX_PROCESSING_INPUT_BYTES
  ) {
    throw new Error("Audio is too large for optional playback processing");
  }
  const encoded = await response.arrayBuffer();
  if (encoded.byteLength > MAX_PROCESSING_INPUT_BYTES) {
    throw new Error("Audio is too large for optional playback processing");
  }
  throwIfAborted(signal);

  const decoder = new AudioContext();
  let decoded: AudioBuffer;
  try {
    decoded = await decoder.decodeAudioData(encoded.slice(0));
  } finally {
    await decoder.close();
  }
  throwIfAborted(signal);

  const outputFrames = Math.max(1, Math.ceil(decoded.length / normalizedPitch));
  if (outputFrames * decoded.numberOfChannels > MAX_RENDERED_SAMPLE_VALUES) {
    throw new Error("Audio is too long for optional playback processing");
  }
  const offline = new OfflineAudioContext(
    decoded.numberOfChannels,
    outputFrames,
    decoded.sampleRate,
  );
  const source = offline.createBufferSource();
  source.buffer = decoded;
  source.playbackRate.value = normalizedPitch;

  const output = connectEffect(offline, source, effect);
  output.connect(offline.destination);
  source.start(0);
  const rendered = await offline.startRendering();
  throwIfAborted(signal);

  const objectUrl = URL.createObjectURL(
    new Blob([encodePcm16Wav(rendered)], { type: "audio/wav" }),
  );
  return {
    url: objectUrl,
    objectUrl,
    pitchCompensation: normalizedPitch,
  };
}

function connectEffect(
  context: OfflineAudioContext,
  source: AudioBufferSourceNode,
  effect: TtsPlaybackEffect,
): AudioNode {
  if (effect === "radio") {
    const highpass = context.createBiquadFilter();
    highpass.type = "highpass";
    highpass.frequency.value = 420;
    highpass.Q.value = 0.8;

    const presence = context.createBiquadFilter();
    presence.type = "peaking";
    presence.frequency.value = 1_650;
    presence.Q.value = 1.2;
    presence.gain.value = 7;

    const lowpass = context.createBiquadFilter();
    lowpass.type = "lowpass";
    lowpass.frequency.value = 3_200;
    lowpass.Q.value = 0.9;

    const saturation = context.createWaveShaper();
    saturation.curve = makeSaturationCurve(2.4);
    saturation.oversample = "2x";
    source
      .connect(highpass)
      .connect(presence)
      .connect(lowpass)
      .connect(saturation);
    return saturation;
  }

  if (effect === "retro") {
    const lowpass = context.createBiquadFilter();
    lowpass.type = "lowpass";
    lowpass.frequency.value = 4_200;
    lowpass.Q.value = 0.7;
    const quantizer = context.createWaveShaper();
    quantizer.curve = makeQuantizerCurve(32);
    quantizer.oversample = "none";
    source.connect(lowpass).connect(quantizer);
    return quantizer;
  }

  return source;
}

function makeSaturationCurve(amount: number): Float32Array {
  const curve = new Float32Array(4_096);
  const normalizer = Math.tanh(amount);
  for (let index = 0; index < curve.length; index += 1) {
    const input = (index / (curve.length - 1)) * 2 - 1;
    curve[index] = Math.tanh(input * amount) / normalizer;
  }
  return curve;
}

function makeQuantizerCurve(steps: number): Float32Array {
  const curve = new Float32Array(4_096);
  for (let index = 0; index < curve.length; index += 1) {
    const input = (index / (curve.length - 1)) * 2 - 1;
    curve[index] = Math.round(input * steps) / steps;
  }
  return curve;
}

function encodePcm16Wav(buffer: AudioBuffer): ArrayBuffer {
  const channels = buffer.numberOfChannels;
  const frameCount = buffer.length;
  const dataBytes = frameCount * channels * 2;
  const wav = new ArrayBuffer(44 + dataBytes);
  const view = new DataView(wav);
  writeAscii(view, 0, "RIFF");
  view.setUint32(4, 36 + dataBytes, true);
  writeAscii(view, 8, "WAVE");
  writeAscii(view, 12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, channels, true);
  view.setUint32(24, buffer.sampleRate, true);
  view.setUint32(28, buffer.sampleRate * channels * 2, true);
  view.setUint16(32, channels * 2, true);
  view.setUint16(34, 16, true);
  writeAscii(view, 36, "data");
  view.setUint32(40, dataBytes, true);

  const channelData = Array.from({ length: channels }, (_, channel) =>
    buffer.getChannelData(channel),
  );
  let offset = 44;
  for (let frame = 0; frame < frameCount; frame += 1) {
    for (let channel = 0; channel < channels; channel += 1) {
      const sample = Math.min(1, Math.max(-1, channelData[channel][frame]));
      view.setInt16(
        offset,
        sample < 0 ? Math.round(sample * 32_768) : Math.round(sample * 32_767),
        true,
      );
      offset += 2;
    }
  }
  return wav;
}

function writeAscii(view: DataView, offset: number, value: string) {
  for (let index = 0; index < value.length; index += 1) {
    view.setUint8(offset + index, value.charCodeAt(index));
  }
}

function throwIfAborted(signal: AbortSignal) {
  if (signal.aborted)
    throw new DOMException("Playback processing cancelled", "AbortError");
}
