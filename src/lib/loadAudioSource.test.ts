import { expect, spyOn, test } from "bun:test";
import { loadAudioSource } from "./loadAudioSource";

test("a History recording finishing after unmount releases its blob", async () => {
  let finish!: (source: string) => void;
  let current = true;
  const revoke = spyOn(URL, "revokeObjectURL").mockImplementation(() => undefined);
  try {
    const pending = loadAudioSource(() => new Promise(resolve => { finish = resolve; }), () => current);
    current = false;
    finish("blob:recording");
    expect(await pending).toBeNull();
    expect(revoke).toHaveBeenCalledWith("blob:recording");
  } finally { revoke.mockRestore(); }
});

test("a current recording is retained and read errors reach the player", async () => {
  expect(await loadAudioSource(async () => "blob:current", () => true)).toBe("blob:current");
  await expect(loadAudioSource(async () => { throw new Error("read failed"); }, () => true))
    .rejects.toThrow("read failed");
});
