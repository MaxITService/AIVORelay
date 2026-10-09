import { afterEach, beforeEach, expect, spyOn, test } from "bun:test";
import type { AppSettings } from "@/bindings";
import { useSettingsStore } from "./settingsStore";
import {
  DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG,
  RECORDING_OVERLAY_STYLE_SETTING_ENTRIES,
} from "@/overlay/recordingOverlayStyleConfig";

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

test("appearance is saved in one call and does not overwrite concurrent unrelated state", async () => {
  const backend = deferred();
  const started = deferred();
  const style = { ...DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG, widthPx: 320 };
  const appearance = Object.fromEntries(RECORDING_OVERLAY_STYLE_SETTING_ENTRIES(style));
  const calls: Array<{ command: string; args: Record<string, unknown> }> = [];
  invokeBackend = (command, args) => {
    calls.push({ command, args });
    started.resolve();
    return backend.promise;
  };
  const change = useSettingsStore.getState().applyRecordingOverlayStyle(style);
  await started.promise;
  expect(useSettingsStore.getState().isUpdating.recording_overlay_appearance).toBe(true);
  expect(useSettingsStore.getState().settings?.recording_overlay_width_px).toBeUndefined();
  useSettingsStore.getState().setSettings({ ...settings(), active_profile_id: "profile-b" });
  backend.resolve({ ...settings(), ...appearance });
  await change;
  expect(calls).toEqual([{ command: "apply_recording_overlay_appearance", args: { appearance } }]);
  expect(useSettingsStore.getState().settings?.active_profile_id).toBe("profile-b");
  expect(useSettingsStore.getState().settings?.recording_overlay_width_px).toBe(320);
  expect(useSettingsStore.getState().isUpdating.recording_overlay_appearance).toBe(false);
});

test("global Chinese script changes persist globally while a custom profile is active", async () => {
  const calls: Array<{ command: string; args: Record<string, unknown> }> = [];
  invokeBackend = async (command, args) => { calls.push({ command, args }); };
  await useSettingsStore.getState().updateSetting("chinese_script", "traditional");
  expect(calls).toEqual([{
    command: "change_chinese_script_setting",
    args: { script: "traditional", profileId: null },
  }]);
  expect(useSettingsStore.getState().settings?.chinese_script).toBe("traditional");
  expect(useSettingsStore.getState().settings?.active_profile_id).toBe("profile-a");
  expect(useSettingsStore.getState().settings?.transcription_profiles).toEqual(settings().transcription_profiles);
  expect(useSettingsStore.getState().isUpdating.chinese_script).toBe(false);
});

test("failed Chinese script save rolls back its preference and preserves concurrent unrelated state", async () => {
  const backend = deferred();
  const started = deferred();
  useSettingsStore.getState().setSettings({ ...settings(), chinese_script: "simplified" });
  invokeBackend = () => { started.resolve(); return backend.promise; };
  const change = useSettingsStore.getState().updateSetting("chinese_script", "traditional", { throwOnError: true });
  await started.promise;
  useSettingsStore.getState().setSettings({
    ...useSettingsStore.getState().settings!, active_profile_id: "profile-b",
  });
  backend.reject(new Error("disk full"));
  await expect(change).rejects.toThrow("disk full");
  expect(useSettingsStore.getState().settings?.chinese_script).toBe("simplified");
  expect(useSettingsStore.getState().settings?.active_profile_id).toBe("profile-b");
  expect(useSettingsStore.getState().isUpdating.chinese_script).toBe(false);
});

test("failed appearance save rejects without partial frontend changes and releases its busy state", async () => {
  invokeBackend = async () => { throw new Error("disk full"); };
  const original = useSettingsStore.getState().settings;
  await expect(useSettingsStore.getState().applyRecordingOverlayStyle(DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG))
    .rejects.toThrow("disk full");
  expect(useSettingsStore.getState().settings).toEqual(original);
  expect(useSettingsStore.getState().isUpdating.recording_overlay_appearance).toBe(false);
});

test("appearance and mode edits cannot interleave with an active appearance save", async () => {
  const backend = deferred();
  const started = deferred();
  const calls: string[] = [];
  invokeBackend = (command) => { calls.push(command); started.resolve(); return backend.promise; };
  const change = useSettingsStore.getState().applyRecordingOverlayStyle(DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG);
  await started.promise;
  const rejected = useSettingsStore.getState().applyRecordingOverlayStyle(DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG);
  await expect(rejected).rejects.toThrow("already in progress");
  for (const [key, value] of [["recording_overlay_width_px", 300], ["recording_overlay_custom_enabled", false]] as const) {
    await expect(useSettingsStore.getState().updateSetting(key, value, { throwOnError: true }))
      .rejects.toThrow("Wait for the overlay appearance update");
  }
  backend.resolve({ ...settings(), ...Object.fromEntries(
    RECORDING_OVERLAY_STYLE_SETTING_ENTRIES(DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG),
  ) });
  await change;
  expect(calls).toEqual(["apply_recording_overlay_appearance"]);
});

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
