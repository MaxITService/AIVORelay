import { expect, test } from "bun:test";
import { initializeInputSystem } from "./initializeInput";

test("a failed input initialization stays retryable and does not start shortcuts", async () => {
  let attempts = 0;
  let shortcutAttempts = 0;
  const commands = {
    initializeEnigo: async () => ++attempts === 1
      ? { status: "error" as const, error: "Input unavailable" }
      : { status: "ok" as const, data: null },
    initializeShortcuts: async () => {
      shortcutAttempts++;
      return { status: "ok" as const, data: null };
    },
  };
  await expect(initializeInputSystem(commands)).rejects.toThrow("Input unavailable");
  expect(shortcutAttempts).toBe(0);
  await initializeInputSystem(commands);
  expect(attempts).toBe(2);
  expect(shortcutAttempts).toBe(1);
});

test("shortcut initialization errors are surfaced", async () => {
  await expect(initializeInputSystem({
    initializeEnigo: async () => ({ status: "ok", data: null }),
    initializeShortcuts: async () => ({ status: "error", error: "Shortcuts unavailable" }),
  })).rejects.toThrow("Shortcuts unavailable");
});
