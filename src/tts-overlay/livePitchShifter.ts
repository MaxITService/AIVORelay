// Delay sweep per crossfade cycle. Longer windows smear speech; shorter ones
// make the crossfade audible as roughness.
export const PITCH_SHIFT_WINDOW_SECONDS = 0.1;
// Below about 17 cents the shift is inaudible, while the modulation period
// (window / |ratio - 1|) and its buffer grow without bound.
const MIN_AUDIBLE_RATIO_DELTA = 0.01;

export const isPitchShiftBypassed = (ratio: number) =>
  !Number.isFinite(ratio) ||
  ratio <= 0 ||
  Math.abs(ratio - 1) < MIN_AUDIBLE_RATIO_DELTA;

/**
 * One modulation cycle for a delay line: channel 0 is the delay time and
 * channel 1 the gain. The delay sweeps linearly so the line reads its input
 * `ratio` times faster (or slower), and wraps where a Hann fade is silent.
 */
export function pitchShiftCycle(
  ratio: number,
  sampleRate: number,
  windowSeconds = PITCH_SHIFT_WINDOW_SECONDS,
) {
  const period = windowSeconds / Math.abs(ratio - 1);
  // Even, so the second line's half-cycle offset is sample-exact.
  const length = Math.max(2, 2 * Math.round((period * sampleRate) / 2));
  const delay = new Float32Array(length);
  const gain = new Float32Array(length);
  for (let i = 0; i < length; i += 1) {
    const phase = i / length;
    delay[i] = windowSeconds * (ratio > 1 ? 1 - phase : phase);
    gain[i] = Math.sin(Math.PI * phase) ** 2;
  }
  return { delay, gain };
}

type ShiftChain = {
  nodes: AudioNode[];
  modulators: AudioBufferSourceNode[];
};

/**
 * Continuous playback pitch shifter made from native Web Audio nodes: two
 * swept delay lines half a cycle apart, crossfaded so each wraps while
 * silent. A single instance spans every scheduled chunk, so it shifts pitch
 * without changing duration and without windowing streamed chunk joins.
 */
export class LivePitchShifter {
  readonly input: GainNode;
  private context: BaseAudioContext;
  private destination: AudioNode;
  private ratio = 1;
  private chain: ShiftChain | null = null;

  constructor(
    context: BaseAudioContext,
    destination: AudioNode = context.destination,
  ) {
    this.context = context;
    this.destination = destination;
    this.input = context.createGain();
    this.input.connect(destination);
  }

  /** Longest time audio can remain inside the delay lines. */
  get latencySeconds() {
    return this.chain ? PITCH_SHIFT_WINDOW_SECONDS : 0;
  }

  setRatio(ratio: number) {
    const next = isPitchShiftBypassed(ratio) ? 1 : ratio;
    if (next === this.ratio) {
      return;
    }
    this.ratio = next;
    this.rebuild();
  }

  /** Drop audio still inside the delay lines, e.g. after a seek. */
  flush() {
    if (this.chain) {
      this.rebuild();
    }
  }

  dispose() {
    this.teardown();
    this.input.disconnect();
  }

  private rebuild() {
    this.teardown();
    this.input.disconnect();
    if (this.ratio === 1) {
      this.input.connect(this.destination);
      return;
    }

    const { delay, gain } = pitchShiftCycle(
      this.ratio,
      this.context.sampleRate,
    );
    const cycle = this.context.createBuffer(
      2,
      delay.length,
      this.context.sampleRate,
    );
    cycle.getChannelData(0).set(delay);
    cycle.getChannelData(1).set(gain);
    const startAt = this.context.currentTime;
    const chain: ShiftChain = { nodes: [], modulators: [] };
    for (const offset of [0, cycle.duration / 2]) {
      const line = this.context.createDelay(PITCH_SHIFT_WINDOW_SECONDS * 1.1);
      line.delayTime.value = 0;
      const fade = this.context.createGain();
      fade.gain.value = 0;
      const split = this.context.createChannelSplitter(2);
      const modulator = this.context.createBufferSource();
      modulator.buffer = cycle;
      modulator.loop = true;
      modulator.connect(split);
      split.connect(line.delayTime, 0);
      split.connect(fade.gain, 1);
      this.input.connect(line);
      line.connect(fade);
      fade.connect(this.destination);
      modulator.start(startAt, offset);
      chain.nodes.push(line, fade, split);
      chain.modulators.push(modulator);
    }
    this.chain = chain;
  }

  private teardown() {
    const chain = this.chain;
    this.chain = null;
    if (!chain) {
      return;
    }
    for (const modulator of chain.modulators) {
      try {
        modulator.stop();
      } catch {
        // A modulator that never started needs no stop.
      }
      modulator.disconnect();
    }
    for (const node of chain.nodes) {
      node.disconnect();
    }
  }
}
