import { expect, mock, test } from "bun:test";
import { GaplessTtsPlayer } from "./GaplessTtsPlayer";

async function until(condition: () => boolean) {
  for (let turn = 0; turn < 100; turn += 1) {
    if (condition()) return;
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
  }
  throw new Error("Playback did not reach the expected state");
}

async function withPlayer(
  inspect: (fixture: {
    player: GaplessTtsPlayer;
    starts: Array<{ buffer: AudioBuffer; offset: number }>;
    requests: string[];
    context: () => { currentTime: number };
    onError: ReturnType<typeof mock>;
  }) => Promise<void>,
  firstDecode?: () => Promise<void>,
) {
  const names = ["AudioContext", "OfflineAudioContext", "window", "fetch", "requestAnimationFrame", "cancelAnimationFrame"] as const;
  const originals = names.map((name) => Object.getOwnPropertyDescriptor(globalThis, name));
  const starts: Array<{ buffer: AudioBuffer; offset: number }> = [];
  const requests: string[] = [];
  const onError = mock();
  let activeContext!: FakeAudioContext;
  let decodeCount = 0;
  const buffer = (processed = false) => ({
    duration: 0.25, length: 6_000, sampleRate: 24_000, numberOfChannels: 1, processed,
  } as unknown as AudioBuffer);
  const node = () => ({
    gain: { value: 1 }, frequency: { value: 0 }, Q: { value: 0 },
    connect(target: unknown) { return target; }, disconnect() {},
  });
  class FakeAudioContext {
    currentTime = 0;
    state = "running";
    destination = {};
    constructor() { activeContext = this; }
    async decodeAudioData() {
      decodeCount += 1;
      if (decodeCount === 1) await firstDecode?.();
      return buffer();
    }
    createGain() { return node(); }
    createBufferSource() {
      const source = {
        buffer: null as AudioBuffer | null, playbackRate: { value: 1 },
        connect() {}, disconnect() {}, stop() {},
        start(_time: number, offset: number) { starts.push({ buffer: source.buffer!, offset }); },
      };
      return source;
    }
    async suspend() { this.state = "suspended"; }
    async resume() { this.state = "running"; }
    async close() { this.state = "closed"; }
  }
  class FakeOfflineAudioContext {
    destination = {};
    createBufferSource() {
      return { buffer: null, playbackRate: { value: 1 }, connect: node().connect, start() {} };
    }
    createBiquadFilter() { return node(); }
    createWaveShaper() { return node(); }
    async startRendering() { return buffer(true); }
  }
  const values = [
    FakeAudioContext, FakeOfflineAudioContext,
    { __TAURI_INTERNALS__: { convertFileSrc: (path: string) => path } },
    async (url: string) => { requests.push(url); return new Response(new Uint8Array([0, 0])); },
    () => 1, () => {},
  ];
  const player = new GaplessTtsPlayer({ onSnapshot() {}, onChunkStart() {}, onCompleted() {}, onError });
  try {
    names.forEach((name, index) => Object.defineProperty(globalThis, name, {
      configurable: true, writable: true, value: values[index],
    }));
    await inspect({ player, starts, requests, context: () => activeContext, onError });
  } finally {
    player.stop();
    names.forEach((name, index) => {
      if (originals[index]) Object.defineProperty(globalThis, name, originals[index]!);
      else Reflect.deleteProperty(globalThis, name);
    });
  }
}

test("paused playback can seek beyond the decode-ahead window without starting audio", async () => {
  await withPlayer(async ({ player, starts, requests }) => {
    player.setChunks(Array.from({ length: 40 }, (_, index) => ({ index, path: `chunk-${index}.wav`, pauseAfterMs: 0 })), 40);
    await player.play();
    expect(starts).toHaveLength(12);
    player.pause();
    expect(player.seek(0.8125)).toBe(0.8125);
    expect(starts).toHaveLength(12);
    expect(requests).not.toContain("chunk-32.wav");
    await player.play();
    expect(requests).toContain("chunk-32.wav");
    expect(starts[12].offset).toBeCloseTo(0.125);
  });
});

test("changing an effect while paused reuses raw audio and preserves position", async () => {
  await withPlayer(async ({ player, starts, requests, context, onError }) => {
    player.setChunks([{ index: 0, path: "first.wav", pauseAfterMs: 0 }], 1);
    await player.play();
    const originalBuffer = starts[0].buffer;
    context().currentTime = 0.12;
    player.pause();
    player.configure(1, "radio", 1);
    expect(starts).toHaveLength(1);
    await player.play();
    expect(starts).toHaveLength(2);
    expect(starts[1].buffer).not.toBe(originalBuffer);
    expect(starts[1].offset).toBeCloseTo(0.1);
    expect(requests).toEqual(["first.wav"]);
    expect(onError).not.toHaveBeenCalled();
  });
});

test("an effect changed during the first decode restarts scheduling without stale audio", async () => {
  let release!: () => void;
  let began!: () => void;
  const started = new Promise<void>((resolve) => { began = resolve; });
  const blocked = new Promise<void>((resolve) => { release = resolve; });
  await withPlayer(async ({ player, starts, onError }) => {
    player.setChunks([{ index: 0, path: "first.wav", pauseAfterMs: 0 }], 1);
    const playback = player.play();
    await started;
    player.configure(1, "radio", 1);
    release();
    await playback;
    await until(() => starts.length > 0);
    expect(starts).toHaveLength(1);
    expect((starts[0].buffer as AudioBuffer & { processed: boolean }).processed).toBe(true);
    expect(onError).not.toHaveBeenCalled();
  }, async () => { began(); await blocked; });
});
