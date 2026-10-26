import { describe, expect, test } from "bun:test";
import { clearOnboardingProgress, parseOnboardingProgress, readOnboardingProgress, resolveOnboardingProgress, saveOnboardingProgress } from "./onboardingProgress";

describe("first-start progress", () => {
  test("restores the local choice and remote configuration steps", () => {
    expect(parseOnboardingProgress('{"step":"local","modelId":"model-a"}')).toEqual({ step: "local", modelId: "model-a" });
    expect(parseOnboardingProgress('{"step":"remote"}')).toEqual({ step: "remote" });
    expect(parseOnboardingProgress('{"step":"remote_ready"}')).toEqual({ step: "remote_ready" });
  });

  test("rejects malformed or unrecognized saved state", () => {
    for (const raw of [null, "{", "null", "[]", '{"step":"welcome"}', '{"step":"local"}', '{"step":"local","modelId":42}', '{"step":"local","modelId":""}']) {
      expect(parseOnboardingProgress(raw)).toBeNull();
    }
  });

  test("does not retain unrelated fields such as credentials", () => {
    expect(parseOnboardingProgress('{"step":"remote","apiKey":"secret"}')).toEqual({ step: "remote" });
  });

  test("permissions take precedence, then the previous local or remote step resumes", () => {
    const local = { step: "local", modelId: "model-a" } as const;
    expect(resolveOnboardingProgress(local, true).mode).toBe("permissions");
    expect(resolveOnboardingProgress(local, false).mode).toBe("local");
    expect(resolveOnboardingProgress({ step: "remote" }, false)).toEqual({ mode: "select", welcomeVariant: "local", showRemoteWizard: true });
    expect(resolveOnboardingProgress({ step: "remote_ready" }, false)).toEqual({ mode: "welcome", welcomeVariant: "remote", showRemoteWizard: false });
  });

  test("persists only the resumable state and clears it on completion", () => {
    const original = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
    const values = new Map<string, string>();
    Object.defineProperty(globalThis, "localStorage", { configurable: true, value: {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => values.set(key, value),
      removeItem: (key: string) => values.delete(key),
    } });
    try {
      saveOnboardingProgress({ step: "remote", apiKey: "secret" } as Parameters<typeof saveOnboardingProgress>[0]);
      expect(readOnboardingProgress()).toEqual({ step: "remote" });
      expect([...values.values()][0]).not.toContain("secret");
      clearOnboardingProgress();
      expect(readOnboardingProgress()).toBeNull();
    } finally {
      if (original) Object.defineProperty(globalThis, "localStorage", original);
      else Reflect.deleteProperty(globalThis, "localStorage");
    }
  });
});
