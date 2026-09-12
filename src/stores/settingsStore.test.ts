import { afterEach, beforeEach, expect, spyOn, test } from "bun:test";
import type { AppSettings } from "@/bindings";
import { useSettingsStore } from "./settingsStore";

const originalWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
const originalState = useSettingsStore.getState();
let invokeBackend: (command: string, args: Record<string, unknown>) => Promise<unknown>;
let errorLog: ReturnType<typeof spyOn>;

function deferred() {
  let resolve!: (value?: unknown) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise((ok, fail) => { resolve = ok; reject = fail; });
  return { promise, resolve, reject };
}

function settings(): AppSettings {
  return {
    active_profile_id: "profile-a",
    post_process_enabled: true,
    transcription_profiles: [
      { id: "profile-a", llm_post_process_enabled: false },
      { id: "profile-b", llm_post_process_enabled: true },
    ],
  } as AppSettings;
}

beforeEach(() => {
  invokeBackend = async () => undefined;
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: { __TAURI_INTERNALS__: { invoke: (command: string, args: Record<string, unknown>) => invokeBackend(command, args) } },
  });
  useSettingsStore.setState({ settings: settings(), isUpdating: {} });
  errorLog = spyOn(console, "error").mockImplementation(() => {});
});

afterEach(() => {
  errorLog.mockRestore();
  useSettingsStore.setState(originalState, true);
  if (originalWindow) Object.defineProperty(globalThis, "window", originalWindow);
  else Reflect.deleteProperty(globalThis, "window");
});

test("queued post-processing change stays attached to its original profile", async () => {
  const blocker = deferred();
  const calls: Array<{ command: string; args: Record<string, unknown> }> = [];
  invokeBackend = async (command, args) => {
    calls.push({ command, args });
    if (command === "change_sidebar_width_setting") await blocker.promise;
  };
  const first = useSettingsStore.getState().updateSetting("sidebar_width", 300);
  const change = useSettingsStore.getState().updateSetting("post_process_enabled", true);
  useSettingsStore.getState().setSettings({
    ...useSettingsStore.getState().settings!, active_profile_id: "profile-b",
  });
  blocker.resolve();
  await Promise.all([first, change]);
  expect(calls.find((call) => call.command === "change_post_process_enabled_setting")?.args)
    .toEqual({ enabled: true, profileId: "profile-a" });
  expect(useSettingsStore.getState().settings?.active_profile_id).toBe("profile-b");
});

test("failed profile write rolls back its target without changing the newly active profile", async () => {
  const backend = deferred();
  const started = deferred();
  invokeBackend = () => { started.resolve(); return backend.promise; };
  const change = useSettingsStore.getState().updateSetting("post_process_enabled", true);
  useSettingsStore.getState().setSettings({
    ...useSettingsStore.getState().settings!, active_profile_id: "profile-b",
  });
  await started.promise;
  backend.reject(new Error("disk full"));
  await change;
  const current = useSettingsStore.getState().settings!;
  expect(current.transcription_profiles!.map((profile) => profile.llm_post_process_enabled))
    .toEqual([false, true]);
  expect(current.active_profile_id).toBe("profile-b");
  expect(current.post_process_enabled).toBe(true);
  expect(useSettingsStore.getState().isUpdating.post_process_enabled).toBe(false);
});

test("rollback of a deleted profile does not change default-profile settings", async () => {
  const backend = deferred();
  const started = deferred();
  invokeBackend = () => { started.resolve(); return backend.promise; };
  const change = useSettingsStore.getState().updateSetting("post_process_enabled", true);
  useSettingsStore.getState().setSettings({
    ...useSettingsStore.getState().settings!,
    active_profile_id: "default",
    transcription_profiles: [],
  });
  await started.promise;
  backend.reject(new Error("profile removed"));
  await change;
  expect(useSettingsStore.getState().settings?.post_process_enabled).toBe(true);
  expect(useSettingsStore.getState().settings?.transcription_profiles).toEqual([]);
});
