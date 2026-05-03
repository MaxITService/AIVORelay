import React from "react";
import { type } from "@tauri-apps/plugin-os";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { HandyShortcut } from "../HandyShortcut";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { Dropdown } from "../../ui/Dropdown";
import { Slider } from "../../ui/Slider";
import { Input } from "../../ui/Input";
import { Textarea } from "../../ui/Textarea";
import { TellMeMore } from "../../ui/TellMeMore";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../../hooks/useSettings";
import { ShowTrayIcon } from "../ShowTrayIcon";
import { ShowTrayShortcutGuide } from "../ShowTrayShortcutGuide";
import { RecordingOverlaySettings } from "./RecordingOverlaySettings";
import type { OSType } from "../../../lib/utils/keyboard";
import {
  formatPreviewHotkeyForDisplay,
  normalizePreviewHotkeyString,
} from "../../../lib/utils/previewHotkeys";
import { Info } from "lucide-react";
import { HotkeyCapture } from "../../ui/HotkeyCapture";

const SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN = 24;
const SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX = 320;
const SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN = -10000;
const SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX = 10000;
const SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN = 320;
const SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX = 2200;
const SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN = 100;
const SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX = 1400;
const LOCAL_PREVIEW_AUTO_FLUSH_INTERVAL_MIN_MS = 1000;
const LOCAL_PREVIEW_AUTO_FLUSH_INTERVAL_MAX_MS = 30000;
const LOCAL_PREVIEW_AUTO_FLUSH_OVERLAP_MIN_MS = 0;
const LOCAL_PREVIEW_AUTO_FLUSH_OVERLAP_MAX_MS = 2000;
const SLIDING_LM_WINDOW_TAIL_WORDS_MIN = 20;
const SLIDING_LM_WINDOW_TAIL_WORDS_MAX = 240;
const DEFAULT_SLIDING_LM_WINDOW_PROMPT =
  "You are inside a speech recognition live preview system.\n" +
  "Rewrite ONLY the editable tail so it fits naturally after the stable context.\n" +
  "Stitch chunk boundaries, fix punctuation and capitalization, remove repeated boundary artifacts and hallucinated tails, and preserve the original meaning and language.\n" +
  "Return ONLY the corrected final text for the editable tail. Do not explain, quote, or use JSON.\n\n" +
  "Language: ${language}\n" +
  "Profile: ${profile_name}\n" +
  "Current app: ${current_app}\n\n" +
  "Stable context before editable tail:\n${stable_context}\n\n" +
  "Current full preview:\n${current_preview}\n\n" +
  "Editable tail to return corrected:\n${editable_tail}\n\n" +
  "Newest deterministic chunk:\n${new_chunk}\n\n" +
  "Deterministic notes:\n${deterministic_notes}";

type PreviewActionButtonConfig = {
  id:
    | "close"
    | "clear"
    | "flush"
    | "process"
    | "insert"
    | "delete_until_dot_or_comma"
    | "delete_until_dot"
    | "delete_last_word";
  title: string;
  description: string;
  hotkeyKey: string;
  showVisualKey?: string;
};

const PREVIEW_ACTION_BUTTON_CONFIGS: PreviewActionButtonConfig[] = [
  {
    id: "close",
    title: "Close (X)",
    description: "Hotkey for the X button. = Full cancel, discard all text, do not insert. This button is always visible.",
    hotkeyKey: "soniox_live_preview_close_hotkey",
  },
  {
    id: "clear",
    title: "Clear all",
    description: "Clears preview text without ending the preview workflow.",
    hotkeyKey: "soniox_live_preview_clear_hotkey",
    showVisualKey: "soniox_live_preview_show_clear_button",
  },
  {
    id: "flush",
    title: "Flush",
    description: "Insert current preview text and keep the workflow running.",
    hotkeyKey: "soniox_live_preview_flush_hotkey",
    showVisualKey: "soniox_live_preview_show_flush_button",
  },
  {
    id: "process",
    title: "Processing via LLM",
    description: "Run manual LLM processing on finalized preview text.",
    hotkeyKey: "soniox_live_preview_process_hotkey",
    showVisualKey: "soniox_live_preview_show_process_button",
  },
  {
    id: "insert",
    title: "Insert",
    description: "Insert text and finish the preview workflow.",
    hotkeyKey: "soniox_live_preview_insert_hotkey",
    showVisualKey: "soniox_live_preview_show_insert_button",
  },
  {
    id: "delete_until_dot_or_comma",
    title: "Delete to . / ,",
    description:
      "Finalize current preview text if needed, delete the last clause back to the previous comma or sentence boundary, then continue recording.",
    hotkeyKey: "soniox_live_preview_delete_until_dot_or_comma_hotkey",
    showVisualKey: "soniox_live_preview_show_delete_until_dot_or_comma_button",
  },
  {
    id: "delete_until_dot",
    title: "Delete to .",
    description:
      "Finalize current preview text if needed, delete the last sentence back to the previous sentence boundary, then continue recording.",
    hotkeyKey: "soniox_live_preview_delete_until_dot_hotkey",
    showVisualKey: "soniox_live_preview_show_delete_until_dot_button",
  },
  {
    id: "delete_last_word",
    title: "Delete last word",
    description:
      "Delete the last word from preview text. Its assigned hotkey works globally; Ctrl+Backspace is a separate preview-window-only option. If recording is active and no interim text is visible, the current live tail is dropped instead of being finalized first.",
    hotkeyKey: "soniox_live_preview_delete_last_word_hotkey",
    showVisualKey: "soniox_live_preview_show_delete_last_word_button",
  },
];

function clampToRange(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, Math.round(value)));
}

interface LivePreviewSubsectionProps {
  title: string;
  description?: string;
  disabled?: boolean;
  children: React.ReactNode;
}

const LivePreviewSubsection: React.FC<LivePreviewSubsectionProps> = ({
  title,
  description,
  disabled = false,
  children,
}) => (
  <section
    className={`px-6 py-4 ${disabled ? "opacity-50 pointer-events-none" : ""}`}
  >
    <div className="mb-3">
      <h3 className="text-xs font-bold uppercase tracking-widest text-[#ff8ebb]">
        {title}
      </h3>
      {description && (
        <p className="mt-1 text-xs leading-relaxed text-[#a0a0a0]">
          {description}
        </p>
      )}
    </div>
    <div className="-mx-6 space-y-1">{children}</div>
  </section>
);

export const UserInterfaceSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting, isUpdating, getSetting, refreshSettings } = useSettings();
  const osKind = type();
  const isWindows = osKind === "windows";
  const hotkeyOsType: OSType =
    osKind === "windows" || osKind === "macos" || osKind === "linux"
      ? osKind
      : "unknown";
  const [capturingPreviewHotkeyKey, setCapturingPreviewHotkeyKey] = React.useState<
    string | null
  >(null);
  const [previewActionsExpanded, setPreviewActionsExpanded] = React.useState(false);
  const [isPreviewDemoOpen, setIsPreviewDemoOpen] = React.useState(false);
  const [slidingLmPromptDraft, setSlidingLmPromptDraft] = React.useState(
    DEFAULT_SLIDING_LM_WINDOW_PROMPT,
  );

  const voiceButtonShowAotToggle =
    (settings as any)?.voice_button_show_aot_toggle ?? false;
  const voiceButtonSingleClickClose =
    (settings as any)?.voice_button_single_click_close ?? false;
  const sonioxLivePreviewEnabled =
    (settings as any)?.soniox_live_preview_enabled ?? false;
  const sonioxLivePreviewPosition =
    ((settings as any)?.soniox_live_preview_position ?? "bottom") as string;
  const sonioxLivePreviewCursorOffsetPx = Number(
    (settings as any)?.soniox_live_preview_cursor_offset_px ?? 96,
  );
  const sonioxLivePreviewCustomXPx = Number(
    (settings as any)?.soniox_live_preview_custom_x_px ?? 240,
  );
  const sonioxLivePreviewCustomYPx = Number(
    (settings as any)?.soniox_live_preview_custom_y_px ?? 120,
  );
  const sonioxLivePreviewSize =
    ((settings as any)?.soniox_live_preview_size ?? "medium") as string;
  const sonioxLivePreviewCustomWidthPx = Number(
    (settings as any)?.soniox_live_preview_custom_width_px ?? 760,
  );
  const sonioxLivePreviewCustomHeightPx = Number(
    (settings as any)?.soniox_live_preview_custom_height_px ?? 200,
  );
  const sonioxLivePreviewTheme =
    ((settings as any)?.soniox_live_preview_theme ?? "main_dark") as string;
  const sonioxLivePreviewOpacityPercent = Number(
    (settings as any)?.soniox_live_preview_opacity_percent ?? 88,
  );
  const sonioxLivePreviewFontColor =
    ((settings as any)?.soniox_live_preview_font_color ?? "#f5f5f5") as string;
  const sonioxLivePreviewInterimFontColor =
    ((settings as any)?.soniox_live_preview_interim_font_color ?? "#f5f5f5") as string;
  const sonioxLivePreviewAccentColor =
    ((settings as any)?.soniox_live_preview_accent_color ?? "#ff4d8d") as string;
  const sonioxLivePreviewInterimOpacityPercent = Number(
    (settings as any)?.soniox_live_preview_interim_opacity_percent ?? 58,
  );
  const sonioxLivePreviewShowDragGrip = Boolean(
    (settings as any)?.soniox_live_preview_show_drag_grip ?? true,
  );
  const localPreviewAutoFlushEnabled = Boolean(
    (settings as any)?.soniox_live_preview_local_auto_flush_enabled ?? true,
  );
  const localPreviewAutoFlushIntervalMs = Number(
    (settings as any)?.soniox_live_preview_local_auto_flush_interval_ms ?? 8000,
  );
  const localPreviewAutoFlushOverlapMs = Number(
    (settings as any)?.soniox_live_preview_local_auto_flush_overlap_ms ?? 750,
  );
  const slidingLmWindowEnabled = Boolean(
    (settings as any)?.soniox_live_preview_sliding_lm_window_enabled ?? false,
  );
  const slidingLmWindowPrompt = String(
    (settings as any)?.soniox_live_preview_sliding_lm_window_prompt ??
      DEFAULT_SLIDING_LM_WINDOW_PROMPT,
  );
  const slidingLmWindowTailWords = Number(
    (settings as any)?.soniox_live_preview_sliding_lm_window_tail_words ?? 80,
  );
  const sonioxLivePreviewCtrlBackspaceDeleteLastWord = Boolean(
    (settings as any)?.soniox_live_preview_ctrl_backspace_delete_last_word ?? true,
  );
  const sonioxLivePreviewBackspaceDeleteLastChar = Boolean(
    (settings as any)?.soniox_live_preview_backspace_delete_last_char ?? true,
  );

  const previewActionsSummary = React.useMemo(() => {
    const parts: string[] = [];
    for (const config of PREVIEW_ACTION_BUTTON_CONFIGS) {
      const raw = String((settings as any)?.[config.hotkeyKey] ?? "");
      const normalized = normalizePreviewHotkeyString(raw);
      if (normalized) {
        const display = formatPreviewHotkeyForDisplay(normalized, hotkeyOsType);
        parts.push(`${config.title}: ${display}`);
      }
    }
    return parts.length > 0 ? parts.join(";  ") : "None set";
  }, [settings, hotkeyOsType]);

  React.useEffect(() => {
    setSlidingLmPromptDraft(slidingLmWindowPrompt);
  }, [slidingLmWindowPrompt]);

  const clearPreviewHotkey = (settingKey: string) => {
    void updateSetting(settingKey as any, "" as any);
    if (capturingPreviewHotkeyKey === settingKey) {
      setCapturingPreviewHotkeyKey(null);
    }
  };

  const handleSpawnVoiceButton = async () => {
    try {
      await invoke("spawn_voice_activation_button_window");
    } catch (error) {
      console.error("Failed to spawn voice activation button window:", error);
      toast.error(String(error));
    }
  };

  const updatePxSetting = (
    key: string,
    value: number,
    min: number,
    max: number,
  ) => {
    void updateSetting(key as any, clampToRange(value, min, max) as any);
  };

  React.useEffect(() => {
    if (!sonioxLivePreviewEnabled) {
      setIsPreviewDemoOpen(false);
    }
  }, [sonioxLivePreviewEnabled]);

  const handleTogglePreviewWindow = async () => {
    try {
      if (isPreviewDemoOpen) {
        await invoke("close_soniox_live_preview_demo_window");
        setIsPreviewDemoOpen(false);
      } else {
        await invoke("preview_soniox_live_preview_window");
        setIsPreviewDemoOpen(true);
      }
    } catch (error) {
      console.error("Failed to toggle live preview demo window:", error);
      const message =
        error instanceof Error
          ? error.message
          : typeof error === "string"
            ? error
            : isPreviewDemoOpen
              ? "Failed to close preview window."
              : "Failed to open preview window.";
      toast.error(message);
    }
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      {/* Sidebar Customization Banner */}
      <div className="rounded-lg border border-[#ff4d8d]/30 bg-[#ff4d8d]/10 p-4">
        <div className="flex items-start gap-3">
          <Info className="w-5 h-5 text-[#ff4d8d] mt-0.5 flex-shrink-0" />
          <div className="space-y-1 text-sm text-text/80">
            <p className="font-medium text-text">
              {t("settings.userInterface.sidebarReorder.title")}
            </p>
            <p>
              {t("settings.userInterface.sidebarReorder.description")}
            </p>
          </div>
        </div>
      </div>

      <SettingsGroup title={t("settings.userInterface.title")}>
        <ShowTrayIcon descriptionMode="tooltip" grouped={true} />
        <ShowTrayShortcutGuide descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>

      <RecordingOverlaySettings />

      {isWindows && (
        <SettingsGroup id="live-preview-settings" title="Live Preview">
          <LivePreviewSubsection title="Overview">
            <div className="px-6">
            <TellMeMore title="Tell me more: Live Preview">
              <div className="space-y-3">
                <p>
                  <strong>Live Preview is a staging window for dictated text before final insertion.</strong>
                  Instead of pasting text into the target app immediately, you can review text in the preview window
                  first and decide when to insert it.
                </p>

                <p>
                  <strong>Basic workflow:</strong>
                </p>
                <ol className="list-decimal list-inside space-y-1 ml-1 opacity-90">
                  <li>Enable <strong>Live Preview Window</strong> in this section.</li>
                  <li>Use <strong>Open Preview</strong> to check the preview window.</li>
                  <li>Start dictation using your normal transcription shortcut.</li>
                  <li>Review confirmed and draft text in the preview window.</li>
                  <li>Use <strong>Insert</strong> when you are ready to send text to the target app.</li>
                </ol>

                <div className="p-3 bg-[#1a1a1a] border border-[#333333] rounded-md space-y-2">
                  <p>
                    <strong>Synchronized with active profile:</strong>
                  </p>
                  <p>
                    This toggle and the active profile's <strong>Output to Preview</strong> are kept in sync.
                    Changing either one updates the other immediately.
                  </p>
                  <p>
                    When you switch profiles, this toggle updates to match the new profile's setting.
                    Non-active profiles can have their own Output to Preview value that takes effect when activated.
                  </p>
                </div>

                <div className="p-3 bg-[#1a1a1a] border border-[#333333] rounded-md space-y-2">
                  <p>
                    <strong>Local model auto flush:</strong>
                  </p>
                  <p>
                    When enabled, Local transcription can update Preview while recording continues.
                    Turn it off to return Local Preview to the older behavior: text appears after recording is stopped or manually finalized.
                  </p>
                </div>
              </div>
            </TellMeMore>
            </div>
          </LivePreviewSubsection>
          <LivePreviewSubsection title="Workflow">
          <SettingContainer
            title="Live Preview Window"
            description="Warning: this changes how app inserts text! Preview window appears, then, text is first displayed here, only at the end of recording it is inserted into target application. This setting is synchronized with the active profile's Output to Preview toggle."
            descriptionMode="inline"
            grouped={true}
          >
            <ToggleSwitch
              checked={sonioxLivePreviewEnabled}
              onChange={async (enabled) => {
                await updateSetting(
                  "soniox_live_preview_enabled" as any,
                  enabled as any,
                );
                const activeProfileId = (settings as any)?.active_profile_id ?? "default";
                if (activeProfileId === "default") {
                  await invoke("change_preview_output_only_enabled_setting", { enabled });
                } else {
                  const profiles = (settings?.transcription_profiles ?? []) as any[];
                  const activeProfile = profiles.find((p: any) => p.id === activeProfileId);
                  if (activeProfile) {
                    await invoke("update_transcription_profile", {
                      payload: {
                        id: activeProfile.id,
                        name: activeProfile.name,
                        language: activeProfile.language,
                        translateToEnglish: activeProfile.translate_to_english,
                        systemPrompt: activeProfile.system_prompt || "",
                        sttPromptOverrideEnabled: activeProfile.stt_prompt_override_enabled ?? false,
                        includeInCycle: activeProfile.include_in_cycle,
                        pushToTalk: activeProfile.push_to_talk,
                        previewOutputOnlyEnabled: enabled,
                        llmSettings: {
                          enabled: activeProfile.llm_post_process_enabled ?? false,
                          promptOverride: activeProfile.llm_prompt_override ?? null,
                          modelOverride: activeProfile.llm_model_override ?? null,
                        },
                        sonioxContextGeneralJson: activeProfile.soniox_context_general_json || "",
                        sonioxContextText: activeProfile.soniox_context_text || "",
                        sonioxContextTerms: activeProfile.soniox_context_terms || [],
                        sonioxLanguageHintsStrict: activeProfile.soniox_language_hints_strict ?? null,
                      },
                    });
                  }
                }
                await refreshSettings();
              }}
              disabled={isUpdating("soniox_live_preview_enabled")}
            />
          </SettingContainer>
          </LivePreviewSubsection>
          <LivePreviewSubsection
            title="Preview Controls"
            disabled={!sonioxLivePreviewEnabled}
          >
          <SettingContainer
            title="Preview of the Preview Window"
            description="I heard you like previews? This application has a preview for the preview window, so you can open a resizable preview window that will help you adjust the looks."
            descriptionMode="inline"
            grouped={true}
          >
            <button
              type="button"
              onClick={handleTogglePreviewWindow}
              className="px-3 py-1.5 bg-[#2b2b2b] hover:bg-[#3c3c3c] border border-[#3c3c3c] rounded-lg text-xs text-gray-200 font-medium transition-colors"
            >
              {isPreviewDemoOpen ? "Close Preview" : "Open Preview To See How it Looks"}
            </button>
          </SettingContainer>
            <div className="px-6">
            <button
              type="button"
              onClick={() => setPreviewActionsExpanded((prev) => !prev)}
              className="w-full flex items-center justify-between gap-2 text-left"
            >
              <div className="min-w-0">
                <h3 className="text-base font-semibold tracking-[0.08em] text-orange-400">
                  Preview Action Buttons
                </h3>
                {previewActionsExpanded ? (
                  <p className="text-xs text-[#a0a0a0] mt-0.5">
                    Configure hotkeys and button visibility for Preview actions.
                    Hotkeys are empty by default. Delete last word also supports a separate focused-window-only Ctrl+Backspace toggle below.
                  </p>
                ) : (
                  <p className="text-xs text-orange-400 mt-0.5 truncate">
                    {previewActionsSummary}
                  </p>
                )}
              </div>
              <svg
                className={`w-4 h-4 text-orange-400 shrink-0 transition-transform duration-200 ${previewActionsExpanded ? "rotate-180" : ""}`}
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M19 9l-7 7-7-7"
                />
              </svg>
            </button>
            {previewActionsExpanded && (
            <div className="mt-3 w-full space-y-3">
              {PREVIEW_ACTION_BUTTON_CONFIGS.map((config) => {
                const hotkeyRaw = String((settings as any)?.[config.hotkeyKey] ?? "");
                const normalizedHotkey = normalizePreviewHotkeyString(hotkeyRaw);
                const showVisual = config.showVisualKey
                  ? Boolean((settings as any)?.[config.showVisualKey] ?? true)
                  : true;

                return (
                  <div
                    key={config.id}
                    className="rounded-md border border-[#303030] bg-[#151515]/70 px-3 py-2"
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0">
                        <div className="text-sm font-semibold text-[#f2f2f2]">
                          {config.title}
                        </div>
                        <div className="text-xs text-[#a0a0a0] mt-0.5">
                          {config.description}
                        </div>
                      </div>
                      {config.showVisualKey && (
                        <div className="flex items-center gap-2 shrink-0">
                          <span className="text-[11px] text-[#a0a0a0]">
                            Show visual button
                          </span>
                          <ToggleSwitch
                            checked={showVisual}
                            onChange={(enabled) =>
                              void updateSetting(
                                config.showVisualKey as any,
                                enabled as any,
                              )
                            }
                            disabled={
                              !sonioxLivePreviewEnabled ||
                              isUpdating(config.showVisualKey as string)
                            }
                          />
                        </div>
                      )}
                    </div>
                    <div className="mt-2">
                      <HotkeyCapture
                        value={normalizedHotkey}
                        isCapturing={capturingPreviewHotkeyKey === config.hotkeyKey}
                        onStartCapture={() => setCapturingPreviewHotkeyKey(config.hotkeyKey)}
                        onCaptured={(hotkey) => {
                          void updateSetting(config.hotkeyKey as any, hotkey as any);
                          setCapturingPreviewHotkeyKey(null);
                        }}
                        onCancel={() => setCapturingPreviewHotkeyKey(null)}
                        onClear={() => clearPreviewHotkey(config.hotkeyKey)}
                        disabled={!sonioxLivePreviewEnabled || isUpdating(config.hotkeyKey)}
                        osType={hotkeyOsType}
                      />
                    </div>
                  </div>
                );
              })}
            </div>
            )}
            </div>
          <SettingContainer
            title="Ctrl+Backspace Deletes Last Word"
            description="When the preview window itself is focused, Ctrl+Backspace deletes the last word from preview text. This does not work from other windows."
            descriptionMode="inline"
            grouped={true}
            disabled={!sonioxLivePreviewEnabled}
          >
            <ToggleSwitch
              checked={sonioxLivePreviewCtrlBackspaceDeleteLastWord}
              onChange={(enabled) =>
                void updateSetting(
                  "soniox_live_preview_ctrl_backspace_delete_last_word" as any,
                  enabled as any,
                )
              }
              disabled={
                !sonioxLivePreviewEnabled ||
                isUpdating("soniox_live_preview_ctrl_backspace_delete_last_word")
              }
            />
          </SettingContainer>
          <SettingContainer
            title="Backspace Deletes Last Character"
            description="When the preview window is focused, plain Backspace removes the last character from the finalized preview text, without stopping the active recording."
            descriptionMode="inline"
            grouped={true}
            disabled={!sonioxLivePreviewEnabled}
          >
            <ToggleSwitch
              checked={sonioxLivePreviewBackspaceDeleteLastChar}
              onChange={(enabled) =>
                void updateSetting(
                  "soniox_live_preview_backspace_delete_last_char" as any,
                  enabled as any,
                )
              }
              disabled={
                !sonioxLivePreviewEnabled ||
                isUpdating("soniox_live_preview_backspace_delete_last_char")
              }
            />
          </SettingContainer>
          </LivePreviewSubsection>
          <LivePreviewSubsection
            title="Window Position & Behavior"
            disabled={!sonioxLivePreviewEnabled}
          >
          <SettingContainer
            title="Show Drag Grip"
            description="Show a dotted grip strip at the top of the preview window so you can drag it. Dragging remembers the new window position."
            descriptionMode="inline"
            grouped={true}
            disabled={!sonioxLivePreviewEnabled}
          >
            <ToggleSwitch
              checked={sonioxLivePreviewShowDragGrip}
              onChange={(enabled) =>
                void updateSetting(
                  "soniox_live_preview_show_drag_grip" as any,
                  enabled as any,
                )
              }
              disabled={
                !sonioxLivePreviewEnabled ||
                isUpdating("soniox_live_preview_show_drag_grip")
              }
            />
          </SettingContainer>
            <div className="px-6">
            <TellMeMore title="Tell me more: Positioning">
              <div className="space-y-2">
                <p>
                  <strong>Near Cursor (Dynamic)</strong> repositions the preview when a new live preview session starts.
                  The window appears above your current cursor.
                </p>
                <p>
                  <strong>Cursor Distance</strong> controls how far above the cursor the preview should appear in Near Cursor mode.
                </p>
                <p>
                  <strong>Custom X/Y (px)</strong> pins the preview to exact screen coordinates instead of following the cursor.
                </p>
                <p>
                  If there is not enough space near screen edges, the app keeps the window inside the active monitor.
                </p>
              </div>
            </TellMeMore>
            </div>
          <SettingContainer
            title="Live Preview Position"
            description="Choose where to place the live preview window."
            descriptionMode="inline"
            grouped={true}
          >
            <Dropdown
              options={[
                { value: "bottom", label: "Bottom" },
                { value: "top", label: "Top" },
                { value: "near_cursor", label: "Near Cursor (Dynamic)" },
                { value: "custom_xy", label: "Custom X/Y (px)" },
              ]}
              selectedValue={sonioxLivePreviewPosition}
              onSelect={(value) =>
                void updateSetting(
                  "soniox_live_preview_position" as any,
                  value as any,
                )
              }
            />
          </SettingContainer>
          <SettingContainer
            title="Cursor Distance (Dynamic Mode)"
            description="Vertical distance from cursor to preview window when using Near Cursor position."
            descriptionMode="inline"
            grouped={true}
            disabled={!sonioxLivePreviewEnabled || sonioxLivePreviewPosition !== "near_cursor"}
          >
            <div className="w-full flex items-center gap-3">
              <input
                type="range"
                min={SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN}
                max={SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX}
                step={1}
                value={clampToRange(
                  sonioxLivePreviewCursorOffsetPx,
                  SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN,
                  SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX,
                )}
                onChange={(event) =>
                  updatePxSetting(
                    "soniox_live_preview_cursor_offset_px",
                    Number.parseInt(event.target.value, 10),
                    SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN,
                    SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX,
                  )
                }
                disabled={
                  !sonioxLivePreviewEnabled ||
                  sonioxLivePreviewPosition !== "near_cursor" ||
                  isUpdating("soniox_live_preview_cursor_offset_px")
                }
                className="flex-grow h-2 rounded-full appearance-none cursor-pointer focus:outline-none focus:ring-2 focus:ring-[#ff4d8d]/40 disabled:opacity-40 disabled:cursor-not-allowed"
                style={{
                  background: `linear-gradient(to right, #ff4d8d ${
                    ((clampToRange(
                      sonioxLivePreviewCursorOffsetPx,
                      SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN,
                      SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX,
                    ) -
                      SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN) /
                      (SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX -
                        SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN)) *
                    100
                  }%, #333333 ${
                    ((clampToRange(
                      sonioxLivePreviewCursorOffsetPx,
                      SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN,
                      SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX,
                    ) -
                      SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN) /
                      (SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX -
                        SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN)) *
                    100
                  }%)`,
                }}
              />
              <Input
                type="number"
                min={SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN}
                max={SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX}
                step={1}
                value={clampToRange(
                  sonioxLivePreviewCursorOffsetPx,
                  SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN,
                  SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX,
                )}
                onChange={(event) => {
                  const parsed = Number.parseInt(event.target.value, 10);
                  if (Number.isNaN(parsed)) {
                    return;
                  }
                  updatePxSetting(
                    "soniox_live_preview_cursor_offset_px",
                    parsed,
                    SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN,
                    SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX,
                  );
                }}
                className="w-24"
                disabled={
                  !sonioxLivePreviewEnabled ||
                  sonioxLivePreviewPosition !== "near_cursor" ||
                  isUpdating("soniox_live_preview_cursor_offset_px")
                }
              />
            </div>
          </SettingContainer>
          {sonioxLivePreviewPosition === "custom_xy" && (
            <>
              <SettingContainer
                title="Custom X (px)"
                description="Absolute X screen coordinate of preview window."
                descriptionMode="inline"
                grouped={true}
                disabled={!sonioxLivePreviewEnabled}
              >
                <div className="w-full flex items-center gap-3">
                  <input
                    type="range"
                    min={SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN}
                    max={SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX}
                    step={1}
                    value={clampToRange(
                      sonioxLivePreviewCustomXPx,
                      SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                      SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                    )}
                    onChange={(event) =>
                      updatePxSetting(
                        "soniox_live_preview_custom_x_px",
                        Number.parseInt(event.target.value, 10),
                        SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                        SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                      )
                    }
                    disabled={
                      !sonioxLivePreviewEnabled ||
                      isUpdating("soniox_live_preview_custom_x_px")
                    }
                    className="flex-grow h-2 rounded-full appearance-none cursor-pointer focus:outline-none focus:ring-2 focus:ring-[#ff4d8d]/40 disabled:opacity-40 disabled:cursor-not-allowed"
                    style={{
                      background: `linear-gradient(to right, #ff4d8d ${
                        ((clampToRange(
                          sonioxLivePreviewCustomXPx,
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                        ) -
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN) /
                          (SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX -
                            SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN)) *
                        100
                      }%, #333333 ${
                        ((clampToRange(
                          sonioxLivePreviewCustomXPx,
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                        ) -
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN) /
                          (SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX -
                            SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN)) *
                        100
                      }%)`,
                    }}
                  />
                  <Input
                    type="number"
                    min={SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN}
                    max={SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX}
                    step={1}
                    value={clampToRange(
                      sonioxLivePreviewCustomXPx,
                      SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                      SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                    )}
                    onChange={(event) => {
                      const parsed = Number.parseInt(event.target.value, 10);
                      if (Number.isNaN(parsed)) {
                        return;
                      }
                      updatePxSetting(
                        "soniox_live_preview_custom_x_px",
                        parsed,
                        SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                        SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                      );
                    }}
                    className="w-24"
                    disabled={
                      !sonioxLivePreviewEnabled ||
                      isUpdating("soniox_live_preview_custom_x_px")
                    }
                  />
                </div>
              </SettingContainer>
              <SettingContainer
                title="Custom Y (px)"
                description="Absolute Y screen coordinate of preview window."
                descriptionMode="inline"
                grouped={true}
                disabled={!sonioxLivePreviewEnabled}
              >
                <div className="w-full flex items-center gap-3">
                  <input
                    type="range"
                    min={SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN}
                    max={SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX}
                    step={1}
                    value={clampToRange(
                      sonioxLivePreviewCustomYPx,
                      SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                      SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                    )}
                    onChange={(event) =>
                      updatePxSetting(
                        "soniox_live_preview_custom_y_px",
                        Number.parseInt(event.target.value, 10),
                        SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                        SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                      )
                    }
                    disabled={
                      !sonioxLivePreviewEnabled ||
                      isUpdating("soniox_live_preview_custom_y_px")
                    }
                    className="flex-grow h-2 rounded-full appearance-none cursor-pointer focus:outline-none focus:ring-2 focus:ring-[#ff4d8d]/40 disabled:opacity-40 disabled:cursor-not-allowed"
                    style={{
                      background: `linear-gradient(to right, #ff4d8d ${
                        ((clampToRange(
                          sonioxLivePreviewCustomYPx,
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                        ) -
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN) /
                          (SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX -
                            SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN)) *
                        100
                      }%, #333333 ${
                        ((clampToRange(
                          sonioxLivePreviewCustomYPx,
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                        ) -
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN) /
                          (SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX -
                            SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN)) *
                        100
                      }%)`,
                    }}
                  />
                  <Input
                    type="number"
                    min={SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN}
                    max={SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX}
                    step={1}
                    value={clampToRange(
                      sonioxLivePreviewCustomYPx,
                      SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                      SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                    )}
                    onChange={(event) => {
                      const parsed = Number.parseInt(event.target.value, 10);
                      if (Number.isNaN(parsed)) {
                        return;
                      }
                      updatePxSetting(
                        "soniox_live_preview_custom_y_px",
                        parsed,
                        SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                        SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                      );
                    }}
                    className="w-24"
                    disabled={
                      !sonioxLivePreviewEnabled ||
                      isUpdating("soniox_live_preview_custom_y_px")
                    }
                  />
                </div>
              </SettingContainer>
            </>
          )}
          </LivePreviewSubsection>
          <LivePreviewSubsection title="Size" disabled={!sonioxLivePreviewEnabled}>
          <SettingContainer
            title="Live Preview Size"
            description="Set the size of the live preview window."
            descriptionMode="inline"
            grouped={true}
            disabled={!sonioxLivePreviewEnabled}
          >
            <Dropdown
              options={[
                { value: "small", label: "Small" },
                { value: "medium", label: "Medium" },
                { value: "large", label: "Large" },
                { value: "custom", label: "Custom (px)" },
              ]}
              selectedValue={sonioxLivePreviewSize}
              onSelect={(value) =>
                void updateSetting("soniox_live_preview_size" as any, value as any)
              }
              disabled={
                !sonioxLivePreviewEnabled || isUpdating("soniox_live_preview_size")
              }
            />
          </SettingContainer>
          {sonioxLivePreviewSize === "custom" && (
            <>
              <SettingContainer
                title="Custom Width (px)"
                description="Manual window width in pixels."
                descriptionMode="inline"
                grouped={true}
                disabled={!sonioxLivePreviewEnabled}
              >
                <div className="w-full flex items-center gap-3">
                  <input
                    type="range"
                    min={SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN}
                    max={SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX}
                    step={1}
                    value={clampToRange(
                      sonioxLivePreviewCustomWidthPx,
                      SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN,
                      SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX,
                    )}
                    onChange={(event) =>
                      updatePxSetting(
                        "soniox_live_preview_custom_width_px",
                        Number.parseInt(event.target.value, 10),
                        SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN,
                        SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX,
                      )
                    }
                    disabled={
                      !sonioxLivePreviewEnabled ||
                      isUpdating("soniox_live_preview_custom_width_px")
                    }
                    className="flex-grow h-2 rounded-full appearance-none cursor-pointer focus:outline-none focus:ring-2 focus:ring-[#ff4d8d]/40 disabled:opacity-40 disabled:cursor-not-allowed"
                    style={{
                      background: `linear-gradient(to right, #ff4d8d ${
                        ((clampToRange(
                          sonioxLivePreviewCustomWidthPx,
                          SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN,
                          SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX,
                        ) -
                          SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN) /
                          (SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX -
                            SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN)) *
                        100
                      }%, #333333 ${
                        ((clampToRange(
                          sonioxLivePreviewCustomWidthPx,
                          SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN,
                          SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX,
                        ) -
                          SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN) /
                          (SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX -
                            SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN)) *
                        100
                      }%)`,
                    }}
                  />
                  <Input
                    type="number"
                    min={SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN}
                    max={SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX}
                    step={1}
                    value={clampToRange(
                      sonioxLivePreviewCustomWidthPx,
                      SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN,
                      SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX,
                    )}
                    onChange={(event) => {
                      const parsed = Number.parseInt(event.target.value, 10);
                      if (Number.isNaN(parsed)) {
                        return;
                      }
                      updatePxSetting(
                        "soniox_live_preview_custom_width_px",
                        parsed,
                        SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN,
                        SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX,
                      );
                    }}
                    className="w-24"
                    disabled={
                      !sonioxLivePreviewEnabled ||
                      isUpdating("soniox_live_preview_custom_width_px")
                    }
                  />
                </div>
              </SettingContainer>
              <SettingContainer
                title="Custom Height (px)"
                description="Manual window height in pixels."
                descriptionMode="inline"
                grouped={true}
                disabled={!sonioxLivePreviewEnabled}
              >
                <div className="w-full flex items-center gap-3">
                  <input
                    type="range"
                    min={SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN}
                    max={SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX}
                    step={1}
                    value={clampToRange(
                      sonioxLivePreviewCustomHeightPx,
                      SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN,
                      SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX,
                    )}
                    onChange={(event) =>
                      updatePxSetting(
                        "soniox_live_preview_custom_height_px",
                        Number.parseInt(event.target.value, 10),
                        SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN,
                        SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX,
                      )
                    }
                    disabled={
                      !sonioxLivePreviewEnabled ||
                      isUpdating("soniox_live_preview_custom_height_px")
                    }
                    className="flex-grow h-2 rounded-full appearance-none cursor-pointer focus:outline-none focus:ring-2 focus:ring-[#ff4d8d]/40 disabled:opacity-40 disabled:cursor-not-allowed"
                    style={{
                      background: `linear-gradient(to right, #ff4d8d ${
                        ((clampToRange(
                          sonioxLivePreviewCustomHeightPx,
                          SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN,
                          SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX,
                        ) -
                          SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN) /
                          (SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX -
                            SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN)) *
                        100
                      }%, #333333 ${
                        ((clampToRange(
                          sonioxLivePreviewCustomHeightPx,
                          SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN,
                          SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX,
                        ) -
                          SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN) /
                          (SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX -
                            SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN)) *
                        100
                      }%)`,
                    }}
                  />
                  <Input
                    type="number"
                    min={SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN}
                    max={SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX}
                    step={1}
                    value={clampToRange(
                      sonioxLivePreviewCustomHeightPx,
                      SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN,
                      SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX,
                    )}
                    onChange={(event) => {
                      const parsed = Number.parseInt(event.target.value, 10);
                      if (Number.isNaN(parsed)) {
                        return;
                      }
                      updatePxSetting(
                        "soniox_live_preview_custom_height_px",
                        parsed,
                        SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN,
                        SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX,
                      );
                    }}
                    className="w-24"
                    disabled={
                      !sonioxLivePreviewEnabled ||
                      isUpdating("soniox_live_preview_custom_height_px")
                    }
                  />
                </div>
              </SettingContainer>
            </>
          )}
          </LivePreviewSubsection>
          <LivePreviewSubsection title="Appearance" disabled={!sonioxLivePreviewEnabled}>
            <div className="px-6">
            <TellMeMore title="Tell me more: Appearance">
              <div className="space-y-2">
                <p>
                  <strong>Theme</strong> selects the base preview palette.
                </p>
                <p>
                  <strong>Transparency</strong> controls panel background opacity.
                </p>
                <p>
                  <strong>Confirmed Text Color</strong> affects stable text that will not change.
                </p>
                <p>
                  <strong>Live Draft Color</strong> affects text that may still change before confirmation.
                </p>
                <p>
                  <strong>Accent Color</strong> changes the header and accent tone.
                </p>
                <p>
                  <strong>Live Draft Opacity</strong> controls how faded the draft text looks before it becomes confirmed.
                </p>
              </div>
            </TellMeMore>
            </div>
          <SettingContainer
            title="Live Preview Theme"
            description="Use the app-matching theme by default, or switch to alternate palettes."
            descriptionMode="inline"
            grouped={true}
            disabled={!sonioxLivePreviewEnabled}
          >
            <Dropdown
              options={[
                { value: "main_dark", label: "Main App Dark" },
                { value: "ocean", label: "Ocean Glass" },
                { value: "light", label: "Light" },
              ]}
              selectedValue={sonioxLivePreviewTheme}
              onSelect={(value) =>
                void updateSetting("soniox_live_preview_theme" as any, value as any)
              }
              disabled={
                !sonioxLivePreviewEnabled ||
                isUpdating("soniox_live_preview_theme")
              }
            />
          </SettingContainer>
          <Slider
            label="Live Preview Transparency"
            description="Controls panel transparency."
            descriptionMode="inline"
            grouped={true}
            min={35}
            max={100}
            step={1}
            value={sonioxLivePreviewOpacityPercent}
            formatValue={(value) => `${Math.round(value)}%`}
            onChange={(value) =>
              void updateSetting(
                "soniox_live_preview_opacity_percent" as any,
                Math.round(value) as any,
              )
            }
            disabled={
              !sonioxLivePreviewEnabled ||
              isUpdating("soniox_live_preview_opacity_percent")
            }
          />
          <SettingContainer
            title="Confirmed Text Color"
            description="Color of text that is already confirmed and will not change."
            descriptionMode="inline"
            grouped={true}
            disabled={!sonioxLivePreviewEnabled}
          >
            <div className="flex items-center gap-3">
              <input
                type="color"
                value={sonioxLivePreviewFontColor}
                onChange={(event) =>
                  void updateSetting(
                    "soniox_live_preview_font_color" as any,
                    event.target.value as any,
                  )
                }
                disabled={
                  !sonioxLivePreviewEnabled ||
                  isUpdating("soniox_live_preview_font_color")
                }
                className="h-8 w-12 rounded border border-[#3c3c3c] bg-transparent disabled:opacity-40"
              />
              <span className="text-xs text-[#a0a0a0] font-mono">
                {sonioxLivePreviewFontColor}
              </span>
            </div>
          </SettingContainer>
          <SettingContainer
            title="Live Draft Color"
            description="Color of text that is still being refined and may change."
            descriptionMode="inline"
            grouped={true}
            disabled={!sonioxLivePreviewEnabled}
          >
            <div className="flex items-center gap-3">
              <input
                type="color"
                value={sonioxLivePreviewInterimFontColor}
                onChange={(event) =>
                  void updateSetting(
                    "soniox_live_preview_interim_font_color" as any,
                    event.target.value as any,
                  )
                }
                disabled={
                  !sonioxLivePreviewEnabled ||
                  isUpdating("soniox_live_preview_interim_font_color")
                }
                className="h-8 w-12 rounded border border-[#3c3c3c] bg-transparent disabled:opacity-40"
              />
              <span className="text-xs text-[#a0a0a0] font-mono">
                {sonioxLivePreviewInterimFontColor}
              </span>
            </div>
          </SettingContainer>
          <SettingContainer
            title="Live Preview Accent Color"
            description="Header and accent color."
            descriptionMode="inline"
            grouped={true}
            disabled={!sonioxLivePreviewEnabled}
          >
            <div className="flex items-center gap-3">
              <input
                type="color"
                value={sonioxLivePreviewAccentColor}
                onChange={(event) =>
                  void updateSetting(
                    "soniox_live_preview_accent_color" as any,
                    event.target.value as any,
                  )
                }
                disabled={
                  !sonioxLivePreviewEnabled ||
                  isUpdating("soniox_live_preview_accent_color")
                }
                className="h-8 w-12 rounded border border-[#3c3c3c] bg-transparent disabled:opacity-40"
              />
              <span className="text-xs text-[#a0a0a0] font-mono">
                {sonioxLivePreviewAccentColor}
              </span>
            </div>
          </SettingContainer>
          <Slider
            label="Live Draft Opacity"
            description="How faded the Live Draft text appears before it becomes confirmed."
            descriptionMode="inline"
            grouped={true}
            min={20}
            max={95}
            step={1}
            value={sonioxLivePreviewInterimOpacityPercent}
            formatValue={(value) => `${Math.round(value)}%`}
            onChange={(value) =>
              void updateSetting(
                "soniox_live_preview_interim_opacity_percent" as any,
                Math.round(value) as any,
              )
            }
            disabled={
              !sonioxLivePreviewEnabled ||
              isUpdating("soniox_live_preview_interim_opacity_percent")
            }
          />
          </LivePreviewSubsection>
          <LivePreviewSubsection
            title="Local Auto Flush (Beta)"
            description="Beta feature. Use at your own risk. This local preview mode is not fully designed yet, and it may change or be removed in a future version."
            disabled={!sonioxLivePreviewEnabled}
          >
          <SettingContainer
            title="Local Model Auto Flush"
            description="For Local transcription with Output to Preview, periodically transcribe recorded audio into the preview window while recording continues. Turn this off to restore the older Local Preview behavior."
            descriptionMode="inline"
            grouped={true}
            disabled={!sonioxLivePreviewEnabled}
          >
            <ToggleSwitch
              checked={localPreviewAutoFlushEnabled}
              onChange={(enabled) =>
                void updateSetting(
                  "soniox_live_preview_local_auto_flush_enabled" as any,
                  enabled as any,
                )
              }
              disabled={
                !sonioxLivePreviewEnabled ||
                isUpdating("soniox_live_preview_local_auto_flush_enabled")
              }
            />
          </SettingContainer>
          <Slider
            label="Local Auto Flush Period"
            description="How often Local Preview tries to process a new audio chunk while recording. Shorter values update the preview sooner but may make chunk boundaries more visible."
            descriptionMode="inline"
            grouped={true}
            min={LOCAL_PREVIEW_AUTO_FLUSH_INTERVAL_MIN_MS}
            max={LOCAL_PREVIEW_AUTO_FLUSH_INTERVAL_MAX_MS}
            step={500}
            value={clampToRange(
              localPreviewAutoFlushIntervalMs,
              LOCAL_PREVIEW_AUTO_FLUSH_INTERVAL_MIN_MS,
              LOCAL_PREVIEW_AUTO_FLUSH_INTERVAL_MAX_MS,
            )}
            formatValue={(value) => `${(value / 1000).toFixed(1)} s`}
            onChange={() => {}}
            onChangeComplete={(value) =>
              void updateSetting(
                "soniox_live_preview_local_auto_flush_interval_ms" as any,
                clampToRange(
                  value,
                  LOCAL_PREVIEW_AUTO_FLUSH_INTERVAL_MIN_MS,
                  LOCAL_PREVIEW_AUTO_FLUSH_INTERVAL_MAX_MS,
                ) as any,
              )
            }
            disabled={
              !sonioxLivePreviewEnabled ||
              !localPreviewAutoFlushEnabled ||
              isUpdating("soniox_live_preview_local_auto_flush_interval_ms")
            }
          />
          <Slider
            label="Local Auto Flush Overlap"
            description="Audio kept as context across Local Preview chunks. Higher overlap can reduce missed words at boundaries, but may increase repeated text."
            descriptionMode="inline"
            grouped={true}
            min={LOCAL_PREVIEW_AUTO_FLUSH_OVERLAP_MIN_MS}
            max={LOCAL_PREVIEW_AUTO_FLUSH_OVERLAP_MAX_MS}
            step={100}
            value={clampToRange(
              localPreviewAutoFlushOverlapMs,
              LOCAL_PREVIEW_AUTO_FLUSH_OVERLAP_MIN_MS,
              LOCAL_PREVIEW_AUTO_FLUSH_OVERLAP_MAX_MS,
            )}
            formatValue={(value) => `${Math.round(value)} ms`}
            onChange={() => {}}
            onChangeComplete={(value) =>
              void updateSetting(
                "soniox_live_preview_local_auto_flush_overlap_ms" as any,
                clampToRange(
                  value,
                  LOCAL_PREVIEW_AUTO_FLUSH_OVERLAP_MIN_MS,
                  LOCAL_PREVIEW_AUTO_FLUSH_OVERLAP_MAX_MS,
                ) as any,
              )
            }
            disabled={
              !sonioxLivePreviewEnabled ||
              !localPreviewAutoFlushEnabled ||
              isUpdating("soniox_live_preview_local_auto_flush_overlap_ms")
            }
          />
          <SettingContainer
            title="Sliding LM Window"
            description="After deterministic auto-flush updates the preview, send the recent editable tail to the configured post-processing LLM provider for stitching, punctuation, and cleanup. Changed text is highlighted in green until the preview resets."
            descriptionMode="inline"
            grouped={true}
            disabled={!sonioxLivePreviewEnabled || !localPreviewAutoFlushEnabled}
          >
            <ToggleSwitch
              checked={slidingLmWindowEnabled}
              onChange={(enabled) =>
                void updateSetting(
                  "soniox_live_preview_sliding_lm_window_enabled" as any,
                  enabled as any,
                )
              }
              disabled={
                !sonioxLivePreviewEnabled ||
                !localPreviewAutoFlushEnabled ||
                isUpdating("soniox_live_preview_sliding_lm_window_enabled")
              }
            />
          </SettingContainer>
          <Slider
            label="Sliding LM Editable Tail"
            description="How many recent words the LLM may rewrite. Older text is sent only as stable context and is not replaced."
            descriptionMode="inline"
            grouped={true}
            min={SLIDING_LM_WINDOW_TAIL_WORDS_MIN}
            max={SLIDING_LM_WINDOW_TAIL_WORDS_MAX}
            step={10}
            value={clampToRange(
              slidingLmWindowTailWords,
              SLIDING_LM_WINDOW_TAIL_WORDS_MIN,
              SLIDING_LM_WINDOW_TAIL_WORDS_MAX,
            )}
            formatValue={(value) => `${Math.round(value)} words`}
            onChange={() => {}}
            onChangeComplete={(value) =>
              void updateSetting(
                "soniox_live_preview_sliding_lm_window_tail_words" as any,
                clampToRange(
                  value,
                  SLIDING_LM_WINDOW_TAIL_WORDS_MIN,
                  SLIDING_LM_WINDOW_TAIL_WORDS_MAX,
                ) as any,
              )
            }
            disabled={
              !sonioxLivePreviewEnabled ||
              !localPreviewAutoFlushEnabled ||
              !slidingLmWindowEnabled ||
              isUpdating("soniox_live_preview_sliding_lm_window_tail_words")
            }
          />
          <SettingContainer
            title="Sliding LM Prompt"
            description="Available variables: ${stable_context}, ${editable_tail}, ${new_chunk}, ${current_preview}, ${deterministic_notes}, ${language}, ${profile_name}, ${current_app}."
            descriptionMode="inline"
            grouped={true}
            disabled={
              !sonioxLivePreviewEnabled ||
              !localPreviewAutoFlushEnabled ||
              !slidingLmWindowEnabled
            }
          >
            <div className="w-full max-w-[520px] space-y-2">
              <Textarea
                value={slidingLmPromptDraft}
                onChange={(event) => setSlidingLmPromptDraft(event.target.value)}
                onBlur={() => {
                  if (slidingLmPromptDraft !== slidingLmWindowPrompt) {
                    void updateSetting(
                      "soniox_live_preview_sliding_lm_window_prompt" as any,
                      slidingLmPromptDraft as any,
                    );
                  }
                }}
                disabled={
                  !sonioxLivePreviewEnabled ||
                  !localPreviewAutoFlushEnabled ||
                  !slidingLmWindowEnabled ||
                  isUpdating("soniox_live_preview_sliding_lm_window_prompt")
                }
                className="w-full min-h-[220px] font-mono text-xs"
              />
              <button
                type="button"
                onClick={() => {
                  setSlidingLmPromptDraft(DEFAULT_SLIDING_LM_WINDOW_PROMPT);
                  void updateSetting(
                    "soniox_live_preview_sliding_lm_window_prompt" as any,
                    DEFAULT_SLIDING_LM_WINDOW_PROMPT as any,
                  );
                }}
                disabled={
                  !sonioxLivePreviewEnabled ||
                  !localPreviewAutoFlushEnabled ||
                  !slidingLmWindowEnabled ||
                  isUpdating("soniox_live_preview_sliding_lm_window_prompt")
                }
                className="px-3 py-1.5 bg-[#2b2b2b] hover:bg-[#3c3c3c] disabled:opacity-50 border border-[#3c3c3c] rounded-lg text-xs text-gray-200 font-medium transition-colors"
              >
                Reset prompt
              </button>
            </div>
          </SettingContainer>
          <div className="px-6 pt-2 text-xs leading-relaxed text-[#8f8f8f]">
            <p className="font-semibold text-[#b8b8b8]">
              What to expect when this is enabled
            </p>
            <p className="mt-1">
              While you keep recording, local audio is periodically sent to the
              local model and appended to the preview window. You should see text
              arrive in small batches instead of waiting until recording stops.
            </p>
            <p className="mt-2">
              This is a sliding-window experiment: each flush keeps a short audio
              overlap for the next chunk and then tries to merge repeated words.
              That helps avoid missing words at chunk boundaries, but short
              periods can make punctuation, capitalization, repeated fragments,
              or silence hallucinations more noticeable.
            </p>
            <p className="mt-2 font-semibold text-[#b8b8b8]">
              Recommended starting points
            </p>
            <ul className="mt-1 list-disc space-y-1 pl-5">
              <li>
                Balanced: 6-8 s period, 700-1000 ms overlap. This usually keeps
                preview updates useful without cutting too much sentence context.
              </li>
              <li>
                Faster feedback: 2.5-4 s period, 900-1200 ms overlap. Use this
                when latency matters more than perfect punctuation.
              </li>
              <li>
                Safer long dictation: 10-12 s period, 500-800 ms overlap. This
                gives the model more context and usually behaves closer to the
                older stop-to-finalize flow.
              </li>
            </ul>
            <p className="mt-2">
              Example: if you speak continuously for a paragraph, a 3 s period may
              show text quickly but split clauses awkwardly. A 7 s period is slower
              but usually gives the model enough context for better commas and
              sentence endings. If repeated words appear, lower the overlap; if
              words disappear at boundaries, raise it.
            </p>
          </div>
          </LivePreviewSubsection>
        </SettingsGroup>
      )}

      {isWindows && (
        <SettingsGroup title="Voice Activation Button">
          <SettingContainer
            title="Spawn Voice Activation Button"
            description="Open a floating on-screen voice activation button window."
            descriptionMode="inline"
            grouped={true}
          >
            <button
              type="button"
              onClick={handleSpawnVoiceButton}
              className="px-3 py-1.5 bg-[#2b2b2b] hover:bg-[#3c3c3c] border border-[#3c3c3c] rounded-lg text-xs text-gray-200 font-medium transition-colors"
            >
              Spawn button
            </button>
          </SettingContainer>
          <SettingContainer
            title="Show AOT Toggle in Button Window"
            description="Show the bottom always-on-top control inside the floating voice button window."
            descriptionMode="inline"
            grouped={true}
          >
            <ToggleSwitch
              checked={voiceButtonShowAotToggle}
              onChange={(enabled) =>
                void updateSetting(
                  "voice_button_show_aot_toggle" as any,
                  enabled as any,
                )
              }
              disabled={isUpdating("voice_button_show_aot_toggle")}
            />
          </SettingContainer>
          <SettingContainer
            title="Pressing x once, not twice closes the window"
            description="When enabled, one click on the x button closes the floating voice button window."
            descriptionMode="inline"
            grouped={true}
          >
            <ToggleSwitch
              checked={voiceButtonSingleClickClose}
              onChange={(enabled) =>
                void updateSetting(
                  "voice_button_single_click_close" as any,
                  enabled as any,
                )
              }
              disabled={isUpdating("voice_button_single_click_close")}
            />
          </SettingContainer>
          <HandyShortcut shortcutId="spawn_button" grouped={true} />
        </SettingsGroup>
      )}

      <SettingsGroup title={t("settings.advanced.window.title")}>
        <ToggleSwitch
          checked={getSetting("remember_window_size") ?? false}
          onChange={(enabled) => updateSetting("remember_window_size", enabled)}
          isUpdating={isUpdating("remember_window_size")}
          label={t("settings.advanced.window.rememberSize.label")}
          description={t("settings.advanced.window.rememberSize.description")}
          descriptionMode="tooltip"
          grouped={true}
        />
        <ToggleSwitch
          checked={getSetting("remember_window_position") ?? false}
          onChange={(enabled) => updateSetting("remember_window_position", enabled)}
          isUpdating={isUpdating("remember_window_position")}
          label={t("settings.advanced.window.rememberPosition.label")}
          description={t("settings.advanced.window.rememberPosition.description")}
          descriptionMode="tooltip"
          grouped={true}
        />
      </SettingsGroup>
    </div>
  );
};
