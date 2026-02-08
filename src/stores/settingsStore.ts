import { create } from "zustand";
import { subscribeWithSelector } from "zustand/middleware";
import type { AppSettings as Settings, AudioDevice } from "@/bindings";
import { commands } from "@/bindings";
import { invoke } from "@tauri-apps/api/core";

interface SettingsStore {
  settings: Settings | null;
  defaultSettings: Settings | null;
  isLoading: boolean;
  isUpdating: Record<string, boolean>;
  audioDevices: AudioDevice[];
  outputDevices: AudioDevice[];
  customSounds: { start: boolean; stop: boolean };
  postProcessModelOptions: Record<string, string[]>;

  // Actions
  initialize: () => Promise<void>;
  loadDefaultSettings: () => Promise<void>;
  updateSetting: <K extends keyof Settings>(
    key: K,
    value: Settings[K],
  ) => Promise<void>;
  resetSetting: (key: keyof Settings) => Promise<void>;
  refreshSettings: () => Promise<void>;
  refreshAudioDevices: () => Promise<void>;
  refreshOutputDevices: () => Promise<void>;
  updateBinding: (id: string, binding: string) => Promise<void>;
  resetBinding: (id: string) => Promise<void>;
  getSetting: <K extends keyof Settings>(key: K) => Settings[K] | undefined;
  isUpdatingKey: (key: string) => boolean;
  playTestSound: (soundType: "start" | "stop") => Promise<void>;
  checkCustomSounds: () => Promise<void>;
  setPostProcessProvider: (providerId: string) => Promise<void>;
  updatePostProcessSetting: (
    settingType: "base_url" | "api_key" | "model",
    providerId: string,
    value: string,
  ) => Promise<void>;
  updatePostProcessBaseUrl: (
    providerId: string,
    baseUrl: string,
  ) => Promise<void>;
  updatePostProcessApiKey: (
    providerId: string,
    apiKey: string,
  ) => Promise<void>;
  updatePostProcessModel: (providerId: string, model: string) => Promise<void>;
  fetchPostProcessModels: (providerId: string) => Promise<string[]>;
  fetchLlmModels: (
    feature: "post_processing" | "ai_replace",
  ) => Promise<string[]>;
  setPostProcessModelOptions: (providerId: string, models: string[]) => void;
  setTranscriptionProvider: (providerId: string) => Promise<void>;
  updateRemoteSttBaseUrl: (baseUrl: string) => Promise<void>;
  updateRemoteSttModelId: (modelId: string) => Promise<void>;
  updateRemoteSttDebugCapture: (enabled: boolean) => Promise<void>;
  updateRemoteSttDebugMode: (mode: string) => Promise<void>;
  setAiReplaceProvider: (providerId: string | null) => Promise<void>;
  updateAiReplaceApiKey: (providerId: string, apiKey: string) => Promise<void>;
  updateAiReplaceModel: (providerId: string, model: string) => Promise<void>;
  setVoiceCommandProvider: (providerId: string | null) => Promise<void>;
  updateVoiceCommandApiKey: (
    providerId: string,
    apiKey: string,
  ) => Promise<void>;
  updateVoiceCommandModel: (providerId: string, model: string) => Promise<void>;

  // Internal state setters
  setSettings: (settings: Settings | null) => void;
  setDefaultSettings: (defaultSettings: Settings | null) => void;
  setLoading: (loading: boolean) => void;
  setUpdating: (key: string, updating: boolean) => void;
  setAudioDevices: (devices: AudioDevice[]) => void;
  setOutputDevices: (devices: AudioDevice[]) => void;
  setCustomSounds: (sounds: { start: boolean; stop: boolean }) => void;
}

// Note: Default settings are now fetched from Rust via commands.getDefaultSettings()
// This ensures platform-specific defaults (like overlay_position, shortcuts, paste_method) work correctly

const DEFAULT_AUDIO_DEVICE: AudioDevice = {
  index: "default",
  name: "Default",
  is_default: true,
};

const settingUpdaters: {
  [K in keyof Settings]?: (value: Settings[K]) => Promise<unknown>;
} = {
  always_on_microphone: (value) =>
    commands.updateMicrophoneMode(value as boolean),
  audio_feedback: (value) =>
    commands.changeAudioFeedbackSetting(value as boolean),
  audio_feedback_volume: (value) =>
    commands.changeAudioFeedbackVolumeSetting(value as number),
  sound_theme: (value) => commands.changeSoundThemeSetting(value as string),
  start_hidden: (value) => commands.changeStartHiddenSetting(value as boolean),
  autostart_enabled: (value) =>
    commands.changeAutostartSetting(value as boolean),
  update_checks_enabled: (value) =>
    commands.changeUpdateChecksSetting(value as boolean),
  push_to_talk: (value) => commands.changePttSetting(value as boolean),
  selected_microphone: (value) =>
    commands.setSelectedMicrophone(
      (value as string) === "Default" || value === null
        ? "default"
        : (value as string),
    ),
  clamshell_microphone: (value) =>
    commands.setClamshellMicrophone(
      (value as string) === "Default" ? "default" : (value as string),
    ),
  selected_output_device: (value) =>
    commands.setSelectedOutputDevice(
      (value as string) === "Default" || value === null
        ? "default"
        : (value as string),
    ),
  recording_retention_period: (value) =>
    commands.updateRecordingRetentionPeriod(value as string),
  translate_to_english: (value) =>
    commands.changeTranslateToEnglishSetting(value as boolean),
  selected_language: (value) =>
    commands.changeSelectedLanguageSetting(value as string),
  overlay_position: (value) =>
    commands.changeOverlayPositionSetting(value as string),
  debug_mode: (value) => commands.changeDebugModeSetting(value as boolean),
  custom_words: (value) => commands.updateCustomWords(value as string[]),
  custom_words_enabled: (value) =>
    commands.changeCustomWordsEnabledSetting(value as boolean),
  word_correction_threshold: (value) =>
    commands.changeWordCorrectionThresholdSetting(value as number),
  paste_method: (value) => commands.changePasteMethodSetting(value as string),
  paste_delay_ms: (value) =>
    commands.changePasteDelayMsSetting(Math.round(value as number)),
  clipboard_handling: (value) =>
    commands.changeClipboardHandlingSetting(value as string),
  history_limit: (value) => commands.updateHistoryLimit(value as number),
  post_process_enabled: (value) =>
    commands.changePostProcessEnabledSetting(value as boolean),
  post_process_selected_prompt_id: (value) =>
    commands.setPostProcessSelectedPrompt(value as string),
  ai_replace_system_prompt: (value) =>
    commands.changeAiReplaceSystemPromptSetting(value as string),
  ai_replace_user_prompt: (value) =>
    commands.changeAiReplaceUserPromptSetting(value as string),
  ai_replace_max_chars: (value) =>
    commands.changeAiReplaceMaxCharsSetting(value as number),
  send_to_extension_enabled: (value) =>
    commands.changeSendToExtensionEnabledSetting(value as boolean),
  send_to_extension_push_to_talk: (value) =>
    commands.changeSendToExtensionPushToTalkSetting(value as boolean),
  send_to_extension_with_selection_enabled: (value) =>
    commands.changeSendToExtensionWithSelectionEnabledSetting(value as boolean),
  send_to_extension_with_selection_push_to_talk: (value) =>
    commands.changeSendToExtensionWithSelectionPushToTalkSetting(
      value as boolean,
    ),
  send_to_extension_with_selection_allow_no_voice: (value) =>
    commands.changeSendToExtensionWithSelectionAllowNoVoiceSetting(
      value as boolean,
    ),
  send_to_extension_with_selection_quick_tap_threshold_ms: (value) =>
    commands.changeSendToExtensionWithSelectionQuickTapThresholdMsSetting(
      value as number,
    ),
  send_to_extension_with_selection_no_voice_system_prompt: (value) =>
    commands.changeSendToExtensionWithSelectionNoVoiceSystemPromptSetting(
      value as string,
    ),
  ai_replace_selection_push_to_talk: (value) =>
    commands.changeAiReplaceSelectionPushToTalkSetting(value as boolean),
  connector_auto_open_enabled: (value) =>
    commands.changeConnectorAutoOpenEnabledSetting(value as boolean),
  connector_auto_open_url: (value) =>
    commands.changeConnectorAutoOpenUrlSetting(value as string),
  connector_port: (value) =>
    commands.changeConnectorPortSetting(value as number),
  connector_password: (value) =>
    commands.changeConnectorPasswordSetting(value as string),
  screenshot_capture_method: (value) =>
    commands.changeScreenshotCaptureMethodSetting(value as any),
  screenshot_capture_command: (value) =>
    commands.changeScreenshotCaptureCommandSetting(value as string),
  screenshot_folder: (value) =>
    commands.changeScreenshotFolderSetting(value as string),
  screenshot_require_recent: (value) =>
    commands.changeScreenshotRequireRecentSetting(value as boolean),
  screenshot_timeout_seconds: (value) =>
    commands.changeScreenshotTimeoutSecondsSetting(value as number),
  screenshot_include_subfolders: (value) =>
    commands.changeScreenshotIncludeSubfoldersSetting(value as boolean),
  send_screenshot_to_extension_enabled: (value) =>
    commands.changeSendScreenshotToExtensionEnabledSetting(value as boolean),
  send_screenshot_to_extension_push_to_talk: (value) =>
    commands.changeSendScreenshotToExtensionPushToTalkSetting(value as boolean),
  screenshot_allow_no_voice: (value) =>
    commands.changeScreenshotAllowNoVoiceSetting(value as boolean),
  screenshot_quick_tap_threshold_ms: (value) =>
    commands.changeScreenshotQuickTapThresholdMsSetting(value as number),
  screenshot_no_voice_default_prompt: (value) =>
    commands.changeScreenshotNoVoiceDefaultPromptSetting(value as string),
  mute_while_recording: (value) =>
    commands.changeMuteWhileRecordingSetting(value as boolean),
  append_trailing_space: (value) =>
    commands.changeAppendTrailingSpaceSetting(value as boolean),
  log_level: (value) => commands.setLogLevel(value as any),
  app_language: (value) => commands.changeAppLanguageSetting(value as string),
  transcription_provider: (value) =>
    commands.changeTranscriptionProviderSetting(value as string),
  vad_threshold: (value) =>
    commands.changeVadThresholdSetting(value as number),
};

// Fork-specific settings not yet present in generated bindings.
(settingUpdaters as any).native_region_capture_mode = (value: any) =>
  invoke("change_native_region_capture_mode_setting", { mode: value });
(settingUpdaters as any).beta_voice_commands_enabled = (value: any) =>
  invoke("change_beta_voice_commands_enabled_setting", { enabled: value });
(settingUpdaters as any).voice_button_show_aot_toggle = (value: any) =>
  invoke("change_voice_button_show_aot_toggle_setting", { enabled: value });
(settingUpdaters as any).voice_button_single_click_close = (value: any) =>
  invoke("change_voice_button_single_click_close_setting", { enabled: value });

// Extended Thinking / Reasoning settings
(settingUpdaters as any).post_process_reasoning_enabled = (value: any) =>
  invoke("change_post_process_reasoning_enabled_setting", { enabled: value });
(settingUpdaters as any).post_process_reasoning_budget = (value: any) =>
  invoke("change_post_process_reasoning_budget_setting", { budget: value });
(settingUpdaters as any).ai_replace_reasoning_enabled = (value: any) =>
  invoke("change_ai_replace_reasoning_enabled_setting", { enabled: value });
(settingUpdaters as any).ai_replace_reasoning_budget = (value: any) =>
  invoke("change_ai_replace_reasoning_budget_setting", { budget: value });
(settingUpdaters as any).voice_command_reasoning_enabled = (value: any) =>
  invoke("change_voice_command_reasoning_enabled_setting", { enabled: value });
(settingUpdaters as any).voice_command_reasoning_budget = (value: any) =>
  invoke("change_voice_command_reasoning_budget_setting", { budget: value });

// Voice Command Center settings
(settingUpdaters as any).voice_command_enabled = (value: any) =>
  invoke("change_voice_command_enabled_setting", { enabled: value });
(settingUpdaters as any).voice_command_llm_fallback = (value: any) =>
  invoke("change_voice_command_llm_fallback_setting", { enabled: value });
(settingUpdaters as any).voice_command_system_prompt = (value: any) =>
  invoke("change_voice_command_system_prompt_setting", { prompt: value });
(settingUpdaters as any).voice_command_template = (value: any) =>
  invoke("change_voice_command_template_setting", { template: value });
(settingUpdaters as any).voice_command_keep_window_open = (value: any) =>
  invoke("change_voice_command_keep_window_open_setting", { enabled: value });
(settingUpdaters as any).voice_command_auto_run = (value: any) =>
  invoke("change_voice_command_auto_run_setting", { enabled: value });
(settingUpdaters as any).voice_command_auto_run_seconds = (value: any) =>
  invoke("change_voice_command_auto_run_seconds_setting", { seconds: value });
(settingUpdaters as any).voice_command_default_threshold = (value: any) =>
  invoke("change_voice_command_default_threshold_setting", {
    threshold: value,
  });
(settingUpdaters as any).voice_commands = (value: any) =>
  invoke("change_voice_commands_setting", { commands: value });

// Transcription Profiles settings
(settingUpdaters as any).active_profile_id = (value: any) =>
  invoke("set_active_profile", { id: value });
(settingUpdaters as any).profile_switch_overlay_enabled = (value: any) =>
  invoke("change_profile_switch_overlay_enabled_setting", { enabled: value });

// Clipboard / Paste settings
(settingUpdaters as any).convert_lf_to_crlf = (value: any) =>
  invoke("change_convert_lf_to_crlf_setting", { enabled: value });

// Text Replacement settings
(settingUpdaters as any).text_replacements_enabled = (value: any) =>
  invoke("change_text_replacements_enabled_setting", { enabled: value });
(settingUpdaters as any).text_replacements = (value: any) =>
  invoke("change_text_replacements_setting", { replacements: value });
(settingUpdaters as any).text_replacements_before_llm = (value: any) =>
  invoke("change_text_replacements_before_llm_setting", { enabled: value });
(settingUpdaters as any).custom_words_ngram_enabled = (value: any) =>
  invoke("change_custom_words_ngram_enabled_setting", { enabled: value });

// UI State settings
(settingUpdaters as any).sidebar_pinned = (value: any) =>
  invoke("change_sidebar_pinned_setting", { pinned: value });
(settingUpdaters as any).sidebar_width = (value: any) =>
  invoke("change_sidebar_width_setting", { width: value });

export const useSettingsStore = create<SettingsStore>()(
  subscribeWithSelector((set, get) => ({
    settings: null,
    defaultSettings: null,
    isLoading: true,
    isUpdating: {},
    audioDevices: [],
    outputDevices: [],
    customSounds: { start: false, stop: false },
    postProcessModelOptions: {},

    // Internal setters
    setSettings: (settings) => set({ settings }),
    setDefaultSettings: (defaultSettings) => set({ defaultSettings }),
    setLoading: (isLoading) => set({ isLoading }),
    setUpdating: (key, updating) =>
      set((state) => ({
        isUpdating: { ...state.isUpdating, [key]: updating },
      })),
    setAudioDevices: (audioDevices) => set({ audioDevices }),
    setOutputDevices: (outputDevices) => set({ outputDevices }),
    setCustomSounds: (customSounds) => set({ customSounds }),

    // Getters
    getSetting: (key) => get().settings?.[key],
    isUpdatingKey: (key) => get().isUpdating[key] || false,

    // Load settings from store
    refreshSettings: async () => {
      try {
        const result = await commands.getAppSettings();
        if (result.status === "ok") {
          const settings = result.data;
          const normalizedSettings: Settings = {
            ...settings,
            always_on_microphone: settings.always_on_microphone ?? false,
            selected_microphone: settings.selected_microphone ?? "Default",
            clamshell_microphone: settings.clamshell_microphone ?? "Default",
            selected_output_device:
              settings.selected_output_device ?? "Default",
          };
          set({ settings: normalizedSettings, isLoading: false });
        } else {
          console.error("Failed to load settings:", result.error);
          set({ isLoading: false });
        }
      } catch (error) {
        console.error("Failed to load settings:", error);
        set({ isLoading: false });
      }
    },

    // Load audio devices
    refreshAudioDevices: async () => {
      try {
        const result = await commands.getAvailableMicrophones();
        if (result.status === "ok") {
          const devicesWithDefault = [
            DEFAULT_AUDIO_DEVICE,
            ...result.data.filter(
              (d) => d.name !== "Default" && d.name !== "default",
            ),
          ];
          set({ audioDevices: devicesWithDefault });
        } else {
          set({ audioDevices: [DEFAULT_AUDIO_DEVICE] });
        }
      } catch (error) {
        console.error("Failed to load audio devices:", error);
        set({ audioDevices: [DEFAULT_AUDIO_DEVICE] });
      }
    },

    // Load output devices
    refreshOutputDevices: async () => {
      try {
        const result = await commands.getAvailableOutputDevices();
        if (result.status === "ok") {
          const devicesWithDefault = [
            DEFAULT_AUDIO_DEVICE,
            ...result.data.filter(
              (d) => d.name !== "Default" && d.name !== "default",
            ),
          ];
          set({ outputDevices: devicesWithDefault });
        } else {
          set({ outputDevices: [DEFAULT_AUDIO_DEVICE] });
        }
      } catch (error) {
        console.error("Failed to load output devices:", error);
        set({ outputDevices: [DEFAULT_AUDIO_DEVICE] });
      }
    },

    // Play a test sound
    playTestSound: async (soundType: "start" | "stop") => {
      try {
        await commands.playTestSound(soundType);
      } catch (error) {
        console.error(`Failed to play test sound (${soundType}):`, error);
      }
    },

    checkCustomSounds: async () => {
      try {
        const sounds = await commands.checkCustomSounds();
        get().setCustomSounds(sounds);
      } catch (error) {
        console.error("Failed to check custom sounds:", error);
      }
    },

    // Update a specific setting
    updateSetting: async <K extends keyof Settings>(
      key: K,
      value: Settings[K],
    ) => {
      const { settings, setUpdating } = get();
      const updateKey = String(key);
      const originalValue = settings?.[key];

      setUpdating(updateKey, true);

      try {
        set((state) => ({
          settings: state.settings ? { ...state.settings, [key]: value } : null,
        }));

        const updater = settingUpdaters[key];
        if (updater) {
          const result = await updater(value);
          if (
            result &&
            typeof result === "object" &&
            "status" in result &&
            (result as any).status === "error"
          ) {
            throw new Error(String((result as any).error));
          }
        } else if (key !== "bindings" && key !== "selected_model") {
          console.warn(`No handler for setting: ${String(key)}`);
        }
      } catch (error) {
        console.error(`Failed to update setting ${String(key)}:`, error);
        if (settings) {
          set({ settings: { ...settings, [key]: originalValue } });
        }
      } finally {
        setUpdating(updateKey, false);
      }
    },

    // Reset a setting to its default value
    resetSetting: async (key) => {
      const { defaultSettings } = get();
      if (defaultSettings) {
        const defaultValue = defaultSettings[key];
        if (defaultValue !== undefined) {
          await get().updateSetting(key, defaultValue as any);
        }
      }
    },

    // Update a specific binding
    updateBinding: async (id, binding) => {
      const { settings, setUpdating } = get();
      const updateKey = `binding_${id}`;
      const originalBinding = settings?.bindings?.[id]?.current_binding;

      setUpdating(updateKey, true);

      try {
        // Optimistic update
        set((state) => ({
          settings: state.settings
            ? {
                ...state.settings,
                bindings: {
                  ...state.settings.bindings,
                  [id]: {
                    ...state.settings.bindings[id]!,
                    current_binding: binding,
                  },
                },
              }
            : null,
        }));

        const result = await commands.changeBinding(id, binding);

        // Check if the command executed successfully
        if (result.status === "error") {
          throw new Error(result.error);
        }

        // Check if the binding change was successful
        if (!result.data.success) {
          throw new Error(result.data.error || "Failed to update binding");
        }
      } catch (error) {
        console.error(`Failed to update binding ${id}:`, error);

        // Rollback on error
        if (originalBinding && get().settings) {
          set((state) => ({
            settings: state.settings
              ? {
                  ...state.settings,
                  bindings: {
                    ...state.settings.bindings,
                    [id]: {
                      ...state.settings.bindings[id]!,
                      current_binding: originalBinding,
                    },
                  },
                }
              : null,
          }));
        }

        // Re-throw to let the caller know it failed
        throw error;
      } finally {
        setUpdating(updateKey, false);
      }
    },

    // Reset a specific binding
    resetBinding: async (id) => {
      const { setUpdating, refreshSettings } = get();
      const updateKey = `binding_${id}`;

      setUpdating(updateKey, true);

      try {
        await commands.resetBinding(id);
        await refreshSettings();
      } catch (error) {
        console.error(`Failed to reset binding ${id}:`, error);
      } finally {
        setUpdating(updateKey, false);
      }
    },

    setPostProcessProvider: async (providerId) => {
      const { settings, setUpdating, refreshSettings } = get();
      const updateKey = "post_process_provider_id";
      const previousId = settings?.post_process_provider_id ?? null;

      setUpdating(updateKey, true);

      if (settings) {
        set((state) => ({
          settings: state.settings
            ? { ...state.settings, post_process_provider_id: providerId }
            : null,
        }));
      }

      try {
        await commands.setPostProcessProvider(providerId);
        await refreshSettings();
      } catch (error) {
        console.error("Failed to set post-process provider:", error);
        if (previousId !== null) {
          set((state) => ({
            settings: state.settings
              ? { ...state.settings, post_process_provider_id: previousId }
              : null,
          }));
        }
      } finally {
        setUpdating(updateKey, false);
      }
    },

    setTranscriptionProvider: async (providerId) => {
      const { settings, setUpdating, refreshSettings } = get();
      const updateKey = "transcription_provider";
      const previousId = settings?.transcription_provider ?? null;

      setUpdating(updateKey, true);

      if (settings) {
        set((state) => ({
          settings: state.settings
            ? { ...state.settings, transcription_provider: providerId as any }
            : null,
        }));
      }

      try {
        await commands.changeTranscriptionProviderSetting(providerId);
        await refreshSettings();
      } catch (error) {
        console.error("Failed to set transcription provider:", error);
        if (previousId !== null) {
          set((state) => ({
            settings: state.settings
              ? {
                  ...state.settings,
                  transcription_provider: previousId as any,
                }
              : null,
          }));
        }
      } finally {
        setUpdating(updateKey, false);
      }
    },

    // Generic updater for post-processing provider settings
    updatePostProcessSetting: async (
      settingType: "base_url" | "api_key" | "model",
      providerId: string,
      value: string,
    ) => {
      const { setUpdating, refreshSettings } = get();
      const updateKey = `post_process_${settingType}:${providerId}`;

      setUpdating(updateKey, true);

      try {
        if (settingType === "base_url") {
          await commands.changePostProcessBaseUrlSetting(providerId, value);
        } else if (settingType === "api_key") {
          await commands.changePostProcessApiKeySetting(providerId, value);
        } else if (settingType === "model") {
          await commands.changePostProcessModelSetting(providerId, value);
        }
        await refreshSettings();
      } catch (error) {
        console.error(
          `Failed to update post-process ${settingType.replace("_", " ")}:`,
          error,
        );
      } finally {
        setUpdating(updateKey, false);
      }
    },

    updatePostProcessBaseUrl: async (providerId, baseUrl) => {
      return get().updatePostProcessSetting("base_url", providerId, baseUrl);
    },

    updatePostProcessApiKey: async (providerId, apiKey) => {
      // Clear cached models when API key changes - user should click refresh after
      set((state) => ({
        postProcessModelOptions: {
          ...state.postProcessModelOptions,
          [providerId]: [],
        },
      }));
      return get().updatePostProcessSetting("api_key", providerId, apiKey);
    },

    updatePostProcessModel: async (providerId, model) => {
      return get().updatePostProcessSetting("model", providerId, model);
    },

    fetchPostProcessModels: async (providerId) => {
      const updateKey = `post_process_models_fetch:${providerId}`;
      const { setUpdating, setPostProcessModelOptions } = get();

      setUpdating(updateKey, true);

      try {
        // Call Tauri backend command instead of fetch
        const result = await commands.fetchPostProcessModels(providerId);
        if (result.status === "ok") {
          setPostProcessModelOptions(providerId, result.data);
          return result.data;
        } else {
          console.error("Failed to fetch models:", result.error);
          return [];
        }
      } catch (error) {
        console.error("Failed to fetch models:", error);
        // Don't cache empty array on error - let user retry
        return [];
      } finally {
        setUpdating(updateKey, false);
      }
    },

    setPostProcessModelOptions: (providerId, models) =>
      set((state) => ({
        postProcessModelOptions: {
          ...state.postProcessModelOptions,
          [providerId]: models,
        },
      })),

    fetchLlmModels: async (feature: "post_processing" | "ai_replace") => {
      const { setUpdating, setPostProcessModelOptions, settings } = get();

      // Get the effective provider ID for this feature
      const effectiveProviderId =
        feature === "ai_replace"
          ? settings?.ai_replace_provider_id ||
            settings?.post_process_provider_id ||
            "openai"
          : settings?.post_process_provider_id || "openai";

      const updateKey = `llm_models_fetch:${feature}:${effectiveProviderId}`;

      setUpdating(updateKey, true);

      try {
        const result = await commands.fetchLlmModels(feature);
        if (result.status === "ok") {
          // Store models under the effective provider ID for this feature
          setPostProcessModelOptions(effectiveProviderId, result.data);
          return result.data;
        } else {
          console.error("Failed to fetch LLM models:", result.error);
          return [];
        }
      } catch (error) {
        console.error("Failed to fetch LLM models:", error);
        return [];
      } finally {
        setUpdating(updateKey, false);
      }
    },

    setAiReplaceProvider: async (providerId: string | null) => {
      const { settings, setUpdating, refreshSettings } = get();
      const updateKey = "ai_replace_provider_id";
      const previousId = settings?.ai_replace_provider_id ?? null;

      setUpdating(updateKey, true);

      if (settings) {
        set((state) => ({
          settings: state.settings
            ? { ...state.settings, ai_replace_provider_id: providerId }
            : null,
        }));
      }

      try {
        await commands.setAiReplaceProvider(providerId);
        await refreshSettings();
      } catch (error) {
        console.error("Failed to set AI Replace provider:", error);
        if (settings) {
          set((state) => ({
            settings: state.settings
              ? { ...state.settings, ai_replace_provider_id: previousId }
              : null,
          }));
        }
      } finally {
        setUpdating(updateKey, false);
      }
    },

    updateAiReplaceApiKey: async (providerId: string, apiKey: string) => {
      const { setUpdating, refreshSettings } = get();
      const updateKey = `ai_replace_api_key:${providerId}`;

      setUpdating(updateKey, true);

      try {
        await commands.changeAiReplaceApiKeySetting(providerId, apiKey);
        await refreshSettings();
      } catch (error) {
        console.error("Failed to update AI Replace API key:", error);
      } finally {
        setUpdating(updateKey, false);
      }
    },

    updateAiReplaceModel: async (providerId: string, model: string) => {
      const { setUpdating, refreshSettings } = get();
      const updateKey = `ai_replace_model:${providerId}`;

      setUpdating(updateKey, true);

      try {
        await commands.changeAiReplaceModelSetting(providerId, model);
        await refreshSettings();
      } catch (error) {
        console.error("Failed to update AI Replace model:", error);
      } finally {
        setUpdating(updateKey, false);
      }
    },

    setVoiceCommandProvider: async (providerId: string | null) => {
      const { settings, setUpdating, refreshSettings } = get();
      const updateKey = "voice_command_provider_id";
      const previousId = settings?.voice_command_provider_id ?? null;

      setUpdating(updateKey, true);

      if (settings) {
        set((state) => ({
          settings: state.settings
            ? { ...state.settings, voice_command_provider_id: providerId }
            : null,
        }));
      }

      try {
        await commands.setVoiceCommandProvider(providerId);
        await refreshSettings();
      } catch (error) {
        console.error("Failed to set Voice Command provider:", error);
        if (settings) {
          set((state) => ({
            settings: state.settings
              ? { ...state.settings, voice_command_provider_id: previousId }
              : null,
          }));
        }
      } finally {
        setUpdating(updateKey, false);
      }
    },

    updateVoiceCommandApiKey: async (providerId: string, apiKey: string) => {
      const { setUpdating, refreshSettings } = get();
      const updateKey = `voice_command_api_key:${providerId}`;

      setUpdating(updateKey, true);

      try {
        await commands.changeVoiceCommandApiKeySetting(providerId, apiKey);
        await refreshSettings();
      } catch (error) {
        console.error("Failed to update Voice Command API key:", error);
      } finally {
        setUpdating(updateKey, false);
      }
    },

    updateVoiceCommandModel: async (providerId: string, model: string) => {
      const { setUpdating, refreshSettings } = get();
      const updateKey = `voice_command_model:${providerId}`;

      setUpdating(updateKey, true);

      try {
        await commands.changeVoiceCommandModelSetting(providerId, model);
        await refreshSettings();
      } catch (error) {
        console.error("Failed to update Voice Command model:", error);
      } finally {
        setUpdating(updateKey, false);
      }
    },

    updateRemoteSttBaseUrl: async (baseUrl) => {
      const { setUpdating, refreshSettings } = get();
      const updateKey = "remote_stt_base_url";

      setUpdating(updateKey, true);
      try {
        await commands.changeRemoteSttBaseUrlSetting(baseUrl);
        await refreshSettings();
      } catch (error) {
        console.error("Failed to update remote STT base URL:", error);
      } finally {
        setUpdating(updateKey, false);
      }
    },

    updateRemoteSttModelId: async (modelId) => {
      const { setUpdating, refreshSettings } = get();
      const updateKey = "remote_stt_model_id";

      setUpdating(updateKey, true);
      try {
        await commands.changeRemoteSttModelIdSetting(modelId);
        await refreshSettings();
      } catch (error) {
        console.error("Failed to update remote STT model ID:", error);
      } finally {
        setUpdating(updateKey, false);
      }
    },

    updateRemoteSttDebugCapture: async (enabled) => {
      const { setUpdating, refreshSettings } = get();
      const updateKey = "remote_stt_debug_capture";

      setUpdating(updateKey, true);
      try {
        await commands.changeRemoteSttDebugCaptureSetting(enabled);
        await refreshSettings();
      } catch (error) {
        console.error("Failed to update remote STT debug capture:", error);
      } finally {
        setUpdating(updateKey, false);
      }
    },

    updateRemoteSttDebugMode: async (mode) => {
      const { setUpdating, refreshSettings } = get();
      const updateKey = "remote_stt_debug_mode";

      setUpdating(updateKey, true);
      try {
        await commands.changeRemoteSttDebugModeSetting(mode);
        await refreshSettings();
      } catch (error) {
        console.error("Failed to update remote STT debug mode:", error);
      } finally {
        setUpdating(updateKey, false);
      }
    },

    // Load default settings from Rust
    loadDefaultSettings: async () => {
      try {
        const result = await commands.getDefaultSettings();
        if (result.status === "ok") {
          set({ defaultSettings: result.data });
        } else {
          console.error("Failed to load default settings:", result.error);
        }
      } catch (error) {
        console.error("Failed to load default settings:", error);
      }
    },

    // Initialize everything
    initialize: async () => {
      const {
        refreshSettings,
        refreshAudioDevices,
        refreshOutputDevices,
        checkCustomSounds,
        loadDefaultSettings,
      } = get();
      await Promise.all([
        loadDefaultSettings(),
        refreshSettings(),
        refreshAudioDevices(),
        refreshOutputDevices(),
        checkCustomSounds(),
      ]);
    },
  })),
);
