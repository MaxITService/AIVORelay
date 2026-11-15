import { expect, mock, spyOn, test } from "bun:test";
import { prepareTtsPlaybackBuffer } from "./ttsPlaybackEffects";

test("reports an audio fetch failure without decoding the error response", async () => {
  const response = new Response("Service unavailable", { status: 503 });
  const fetchMock = spyOn(globalThis, "fetch").mockResolvedValue(response);
  const readBody = spyOn(response, "arrayBuffer");
  const decodeAudioData = mock(async (): Promise<AudioBuffer> => {
    throw new Error("An HTTP error response must not reach the audio decoder");
  });
  const decoder = { decodeAudioData } as unknown as AudioContext;
  const signal = new AbortController().signal;
  const sourceUrl = "https://audio.example.test/chunk.wav";

  try {
    await expect(
      prepareTtsPlaybackBuffer(sourceUrl, 1, "none", decoder, signal),
    ).rejects.toThrow("Unable to load audio for playback (503)");

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledWith(sourceUrl, { signal });
    expect(readBody).not.toHaveBeenCalled();
    expect(decodeAudioData).not.toHaveBeenCalled();
  } finally {
    fetchMock.mockRestore();
    readBody.mockRestore();
  }
});

test("rejects oversized audio before reading its body or starting decoding", async () => {
  const response = new Response(new Uint8Array([0, 0]), {
    headers: { "content-length": String(64 * 1024 * 1024 + 1) },
  });
  const fetchMock = spyOn(globalThis, "fetch").mockResolvedValue(response);
  const readBody = spyOn(response, "arrayBuffer");
  const decodeAudioData = mock(async (): Promise<AudioBuffer> => {
    throw new Error("Oversized audio must not reach the decoder");
  });
  const decoder = { decodeAudioData } as unknown as AudioContext;

  try {
    await expect(
      prepareTtsPlaybackBuffer(
        "https://audio.example.test/oversized.wav",
        1,
        "none",
        decoder,
        new AbortController().signal,
      ),
    ).rejects.toThrow("Audio is too large for playback processing");
    expect(readBody).not.toHaveBeenCalled();
    expect(decodeAudioData).not.toHaveBeenCalled();
  } finally {
    fetchMock.mockRestore();
    readBody.mockRestore();
  }
});

test("preserves a network failure without attempting audio decoding", async () => {
  const networkError = new TypeError("Failed to fetch");
  const fetchMock = spyOn(globalThis, "fetch").mockRejectedValue(networkError);
  const decodeAudioData = mock(async (): Promise<AudioBuffer> => {
    throw new Error("A failed fetch must not reach the decoder");
  });
  const decoder = { decodeAudioData } as unknown as AudioContext;
  const signal = new AbortController().signal;
  const sourceUrl = "https://audio.example.test/offline.wav";

  try {
    await expect(
      prepareTtsPlaybackBuffer(sourceUrl, 1, "none", decoder, signal),
    ).rejects.toBe(networkError);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledWith(sourceUrl, { signal });
    expect(decodeAudioData).not.toHaveBeenCalled();
  } finally {
    fetchMock.mockRestore();
  }
});

test("discards audio that finishes decoding after playback was cancelled", async () => {
  const fetchMock = spyOn(globalThis, "fetch").mockResolvedValue(
    new Response(new Uint8Array([0, 0])),
  );
  let resolveDecode!: (buffer: AudioBuffer) => void;
  const decoded = new Promise<AudioBuffer>((resolve) => {
    resolveDecode = resolve;
  });
  let markDecodeStarted!: () => void;
  const decodeStarted = new Promise<void>((resolve) => {
    markDecodeStarted = resolve;
  });
  const decodeAudioData = mock(() => {
    markDecodeStarted();
    return decoded;
  });
  const decoder = { decodeAudioData } as unknown as AudioContext;
  const controller = new AbortController();

  try {
    const preparation = prepareTtsPlaybackBuffer(
      "https://audio.example.test/cancelled.wav",
      1,
      "none",
      decoder,
      controller.signal,
    );
    await decodeStarted;
    controller.abort();
    resolveDecode({} as AudioBuffer);

    await expect(preparation).rejects.toMatchObject({ name: "AbortError" });
    expect(decodeAudioData).toHaveBeenCalledTimes(1);
  } finally {
    fetchMock.mockRestore();
  }
});
