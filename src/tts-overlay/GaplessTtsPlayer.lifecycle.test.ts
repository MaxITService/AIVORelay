import { expect, mock, test } from "bun:test";
import { GaplessTtsPlayer } from "./GaplessTtsPlayer";

async function withPendingDecode(
  inspect: (fixture: {
    player: GaplessTtsPlayer;
    decodeStarted: Promise<void>;
    finishDecode: () => void;
    failDecode: (error: Error) => void;
    starts: number[];
    onError: ReturnType<typeof mock>;
    onCompleted: ReturnType<typeof mock>;
    fetchAudio: ReturnType<typeof mock>;
  }) => Promise<void>,
) {
  const globals = [
    "AudioContext", "window", "fetch", "requestAnimationFrame", "cancelAnimationFrame",
  ] as const;
  const original = globals.map(name => Object.getOwnPropertyDescriptor(globalThis, name));
  const starts: number[] = [];
  let markDecodeStarted!: () => void;
  const decodeStarted = new Promise<void>(resolve => { markDecodeStarted = resolve; });
  let resolveDecode!: (buffer: AudioBuffer) => void;
  let rejectDecode!: (error: Error) => void;
  const decoded = new Promise<AudioBuffer>((resolve, reject) => {
    resolveDecode = resolve;
    rejectDecode = reject;
  });

  class FakeAudioContext {
    currentTime = 0;
    state = "running";
    destination = {};

    decodeAudioData() {
      markDecodeStarted();
      return decoded;
    }

    createBufferSource() {
      return {
        playbackRate: { value: 1 },
        connect() {},
        disconnect() {},
        stop() {},
        start(time: number) { starts.push(time); },
      };
    }

    async suspend() { this.state = "suspended"; }
    async resume() { this.state = "running"; }
    async close() { this.state = "closed"; }
  }

  const onError = mock();
  const onCompleted = mock();
  const fetchAudio = mock(async () => new Response(new Uint8Array([0, 0])));
  const player = new GaplessTtsPlayer({
    onSnapshot() {},
    onChunkStart() {},
    onError,
    onCompleted,
  });
  const replacements = [
    FakeAudioContext,
    { __TAURI_INTERNALS__: { convertFileSrc: (path: string) => path } },
    fetchAudio,
    () => 1,
    () => {},
  ];
  try {
    globals.forEach((name, index) => {
      Object.defineProperty(globalThis, name, {
        configurable: true,
        writable: true,
        value: replacements[index],
      });
    });
    player.setChunks([{ index: 0, path: "first.wav", pauseAfterMs: 0 }], 1);
    await inspect({
      player,
      decodeStarted,
      finishDecode: () => resolveDecode({ duration: 0.25 } as AudioBuffer),
      failDecode: rejectDecode,
      starts,
      onError,
      onCompleted,
      fetchAudio,
    });
  } finally {
    player.stop();
    globals.forEach((name, index) => {
      const descriptor = original[index];
      if (descriptor) Object.defineProperty(globalThis, name, descriptor);
      else Reflect.deleteProperty(globalThis, name);
    });
  }
}

test("stopping during decoding prevents late audio from starting or reporting completion", async () => {
  await withPendingDecode(async ({ player, decodeStarted, finishDecode, starts, onError, onCompleted }) => {
    const playback = player.play();
    await decodeStarted;
    player.stop();
    finishDecode();
    await playback;

    expect(starts).toEqual([]);
    expect(onError).not.toHaveBeenCalled();
    expect(onCompleted).not.toHaveBeenCalled();
  });
});

test("pausing during decoding keeps audio silent until an explicit resume", async () => {
  await withPendingDecode(async ({ player, decodeStarted, finishDecode, starts, onError }) => {
    const playback = player.play();
    await decodeStarted;
    player.pause();
    finishDecode();
    await playback;

    expect(starts).toEqual([]);
    await player.play();
    expect(starts).toEqual([0.02]);
    expect(onError).not.toHaveBeenCalled();
  });
});

test("a decoder failure reports the affected chunk once and stops subsequent audio loading", async () => {
  await withPendingDecode(async ({ player, decodeStarted, failDecode, starts, onError, onCompleted, fetchAudio }) => {
    player.setChunks([
      { index: 0, path: "broken.wav", pauseAfterMs: 0 },
      { index: 1, path: "next.wav", pauseAfterMs: 0 },
    ], 2);
    const playback = player.play();
    await decodeStarted;
    const error = new Error("Invalid encoded audio");
    failDecode(error);
    await playback;

    expect(onError).toHaveBeenCalledTimes(1);
    expect(onError).toHaveBeenCalledWith(error, 0);
    expect(fetchAudio).toHaveBeenCalledTimes(1);
    expect(starts).toEqual([]);
    expect(onCompleted).not.toHaveBeenCalled();
  });
});
