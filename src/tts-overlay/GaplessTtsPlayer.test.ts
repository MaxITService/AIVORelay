import { expect, test } from "bun:test";
import { GaplessTtsPlayer } from "./GaplessTtsPlayer";

// Exercise scheduling through the public player API with a controllable audio
// clock. In particular, decoding can finish inside the 20 ms startup margin.
async function scheduleChunks(
  secondDecodeTime: number,
  pauseAfterMs = 0,
  playbackRate = 1,
) {
  const globals = [
    "AudioContext",
    "window",
    "fetch",
    "requestAnimationFrame",
    "cancelAnimationFrame",
  ] as const;
  const original = globals.map((name) =>
    Object.getOwnPropertyDescriptor(globalThis, name),
  );
  const starts: number[] = [];
  const rates: number[] = [];
  let contextOptions: AudioContextOptions | undefined;
  let decodeCount = 0;

  class FakeAudioContext {
    currentTime = 0;
    state = "running";
    destination = {};

    constructor(options: AudioContextOptions) {
      contextOptions = options;
    }

    async decodeAudioData() {
      decodeCount += 1;
      if (decodeCount === 2) {
        this.currentTime = secondDecodeTime;
      }
      return { duration: 0.25 };
    }

    createBufferSource() {
      const playbackRate = { value: 1 };
      return {
        playbackRate,
        connect() {},
        disconnect() {},
        stop() {},
        start(time: number) {
          starts.push(time);
          rates.push(playbackRate.value);
        },
      };
    }

    async close() {
      this.state = "closed";
    }
  }

  const replacements = [
    FakeAudioContext,
    { __TAURI_INTERNALS__: { convertFileSrc: (path: string) => path } },
    async () => new Response(new Uint8Array([0, 0])),
    () => 1,
    () => {},
  ];
  const player = new GaplessTtsPlayer({
    onSnapshot() {},
    onChunkStart() {},
    onCompleted() {},
    onError(error) { throw error; },
  });
  try {
    globals.forEach((name, index) => {
      Object.defineProperty(globalThis, name, {
        configurable: true,
        writable: true,
        value: replacements[index],
      });
    });
    player.configure(1, "none", playbackRate);
    player.setChunks([
      { index: 0, path: "first.wav", pauseAfterMs },
      { index: 1, path: "second.wav", pauseAfterMs: 0 },
    ], 2);
    await player.play();
    return { starts, rates, contextOptions };
  } finally {
    player.stop();
    globals.forEach((name, index) => {
      const descriptor = original[index];
      if (descriptor) {
        Object.defineProperty(globalThis, name, descriptor);
      } else {
        Reflect.deleteProperty(globalThis, name);
      }
    });
  }
}

test("stream playback decodes at the provider rate to avoid per-chunk resampling", async () => {
  const { starts, contextOptions } = await scheduleChunks(0);
  expect(contextOptions?.sampleRate).toBe(24_000);
  expect(starts).toEqual([0.02, 0.27]);
});

test("a chunk ready 10 ms before the join starts without inserted silence", async () => {
  const { starts } = await scheduleChunks(0.26);
  expect(starts).toEqual([0.02, 0.27]);
});

test("an actual underrun gets startup lead time", async () => {
  const { starts } = await scheduleChunks(0.30);
  expect(starts[1]).toBeCloseTo(0.32, 10);
});

test("intentional pauses and faster playback retain their exact join", async () => {
  const { starts, rates } = await scheduleChunks(0.235, 100, 2);
  expect(starts[1]).toBeCloseTo(0.245, 10);
  expect(rates).toEqual([2, 2]);
});
