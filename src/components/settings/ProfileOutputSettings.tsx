import React, { useEffect, useRef, useState } from "react";
import { Trans, useTranslation } from "react-i18next";
import { type as getOsType } from "@tauri-apps/plugin-os";
import { AlertTriangle, ChevronDown } from "lucide-react";
import { WindowDetails } from "../ui/WindowDetails";
import { Dropdown, type DropdownOption } from "../ui/Dropdown";
import { HotkeyCapture } from "../ui/HotkeyCapture";
import { Input } from "../ui/Input";
import { SettingContainer } from "../ui/SettingContainer";
import { TellMeMore } from "../ui/TellMeMore";
import { useSettings } from "../../hooks/useSettings";
import { useWindowState } from "../../hooks/useWindowState";
import type { Settings } from "../../stores/settingsStore";
import { sessionToast as toast } from "@/lib/sessionToast";
import { formatKeyCombination, type OSType } from "@/lib/utils/keyboard";
import { isSupportedOutputKey, type ProfileOutputSettings as OutputSettings } from "@/lib/profileOutputSettings";
import { openDefaultProfileOutputSettings } from "@/lib/transcriptionProfileNavigation";
import type { ModelInfo } from "@/bindings";
import type { SttModelSelection } from "@/lib/sttModelSelection";
import { navigateToSettingsAnchor, scrollAndFocusAnchor } from "@/lib/anchorNavigation";
import { getShortcutAnchorId, getShortcutSettingsSection } from "@/lib/shortcutAnchors";
import { useNavigationStore } from "@/stores/navigationStore";
import { buffersOutputForKeyActions, getOutputProviderInfo } from "@/lib/postProcessingAvailability";
import { ProfileOutputPlanPanel } from "./ProfileOutputPlanPanel";

export const getGlobalProfileOutputDefaults = (settings: Settings | null | undefined): OutputSettings => ({
    paste_method: settings?.paste_method ?? "ctrl_v",
    clipboard_handling: settings?.clipboard_handling ?? "restore_plain_text",
    clipboard_history_allowed: settings?.clipboard_history_allowed ?? true,
    convert_lf_to_crlf: settings?.convert_lf_to_crlf ?? true,
    paste_delay_ms: settings?.paste_delay_ms ?? 60,
    check_active_window: settings?.transcription_output_defaults?.check_active_window ?? false,
    live_window_resume_delay_ms: settings?.transcription_output_defaults?.live_window_resume_delay_ms ?? 500,
    live_window_stop_action: settings?.transcription_output_defaults?.live_window_stop_action ?? "preview",
    live_window_wait_limit_secs: settings?.transcription_output_defaults?.live_window_wait_limit_secs ?? 0,
    live_window_wait_limit_action: settings?.transcription_output_defaults?.live_window_wait_limit_action ?? "preview",
    auto_submit: settings?.auto_submit ?? false,
    auto_submit_key: settings?.auto_submit_key ?? "enter",
    before_insert_key: settings?.transcription_output_defaults?.before_insert_key ?? "",
    before_insert_delay_ms: settings?.transcription_output_defaults?.before_insert_delay_ms ?? 150,
    recording_start_key: settings?.transcription_output_defaults?.recording_start_key ?? "",
    recording_start_delay_ms: settings?.transcription_output_defaults?.recording_start_delay_ms ?? 150,
    auto_submit_delay_ms: settings?.transcription_output_defaults?.auto_submit_delay_ms ?? 50,
    cancel_key_after_early_open: settings?.transcription_output_defaults?.cancel_key_after_early_open ?? "",
});

export interface ProfileOutputSettingsFieldsProps {
  scopeKey: string;
  values: OutputSettings;
  defaults: OutputSettings;
  isDefault?: boolean;
  disabled?: boolean;
  onChange: (patch: Partial<OutputSettings>) => void | Promise<void>;
  onDraftChange?: (patch: Partial<OutputSettings>) => void;
  onValidationChange?: (invalid: boolean) => void;
  /** The profile's effective speech model; decides whether text is streamed live. */
  sttSelection?: SttModelSelection;
  localModels?: readonly ModelInfo[];
  /** The profile keeps its text in Preview instead of inserting it. */
  previewOutputOnly?: boolean;
}

type DelayKey = "paste_delay_ms" | "before_insert_delay_ms" | "recording_start_delay_ms" | "auto_submit_delay_ms" | "live_window_resume_delay_ms";
type DelayDraft = { raw: string; badInput: boolean };
type DelayDrafts = Partial<Record<DelayKey, DelayDraft>>;

const isDelayDraftInvalid = ({ raw, badInput }: DelayDraft): boolean => {
  const number = Number(raw);
  return badInput || (raw.trim() !== "" && (!Number.isFinite(number) || number < 0 || number > 5000));
};

export const ProfileOutputSettingsFields: React.FC<ProfileOutputSettingsFieldsProps> = (props) => (
  <ProfileOutputSettingsFieldsContent key={props.scopeKey} {...props} />
);

const SectionHeading: React.FC<{ children: React.ReactNode; hint?: string; first?: boolean }> = ({ children, hint, first = false }) => (
  <>
    <h3 className={`mx-6 mb-1 text-[11px] font-bold uppercase tracking-widest text-[#ff4d8d]/80 ${first ? "mt-2" : "mt-4 border-t border-mid-gray/15 pt-3"}`}>{children}</h3>
    {hint && <p className="mx-6 mb-1 text-xs text-mid-gray">{hint}</p>}
  </>
);

/** Program defaults: what a profile uses for a field it does not set. Profiles never follow the Default profile. */
export const programOutputDefaults = (): OutputSettings => ({
  ...getGlobalProfileOutputDefaults(null), paste_method: getOsType() === "linux" ? "direct" : "ctrl_v",
});

/** A new profile starts as a copy of the Default profile's output, without its key presses. */
export const copyDefaultProfileOutput = (defaultProfile: OutputSettings): OutputSettings => ({
  ...defaultProfile, auto_submit: false, before_insert_key: "", recording_start_key: "", cancel_key_after_early_open: "",
});

/** The values a profile's unset fields resolve to. */
export const profileOutputDefaults = (defaultProfile: OutputSettings, isDefault: boolean): OutputSettings =>
  isDefault ? defaultProfile : programOutputDefaults();

/** Some key press runs around insertion (the keys panel is shown only when text is inserted). */
export const profileKeyActionsEnabled = (values: OutputSettings, defaults: OutputSettings): boolean => {
  const effective = <K extends keyof OutputSettings>(key: K): OutputSettings[K] => values[key] ?? defaults[key];
  return effective("paste_method") !== "none"
    && ((effective("auto_submit") ?? false) || (effective("before_insert_key") ?? "") !== ""
      || (effective("recording_start_key") ?? "") !== "");
};

export const KeyActionsBadge: React.FC = () => {
  const { t } = useTranslation();
  return <span className="flex shrink-0 items-center gap-1.5 text-[10px] text-emerald-400 whitespace-nowrap"><span className="h-2 w-2 shrink-0 rounded-full bg-emerald-400 shadow-[0_0_7px_rgba(52,211,153,0.8)]" aria-hidden="true" />{t("profileOutput.active")}</span>;
};

const openRepasteShortcut = () => navigateToSettingsAnchor({
  activateSection: () => useNavigationStore.getState().setSection(getShortcutSettingsSection("repaste_last")),
  targetId: getShortcutAnchorId("repaste_last"),
  block: "center",
});

const OutputDependencies: React.FC<{ children: React.ReactNode; nested?: boolean }> = ({ children, nested = false }) => (
  <div className={`${nested ? "ml-2" : "ml-4"} mb-1 min-w-0 border-l border-mid-gray/15 [&>div[id]]:pl-4 [&>div[id]>div:last-child]:min-w-0 [&>div[id]>div:last-child]:max-w-full`}>
    {children}
  </div>
);

const ProfileOutputSettingsFieldsContent: React.FC<ProfileOutputSettingsFieldsProps> = ({
  scopeKey, values, defaults: defaultProfile, isDefault = false, disabled = false, onChange, onDraftChange, onValidationChange,
  sttSelection, localModels, previewOutputOnly,
}) => {
  const { t } = useTranslation();
  const defaults = profileOutputDefaults(defaultProfile, isDefault);
  const { settings } = useSettings();
  const [capturing, setCapturing] = useState(false);
  const [customKeyRequested, setCustomKeyRequested] = useState(false);
  const [startCapturing, setStartCapturing] = useState(false);
  const [startCustomRequested, setStartCustomRequested] = useState(false);
  const [cancelCapturing, setCancelCapturing] = useState(false);
  const [cancelCustomRequested, setCancelCustomRequested] = useState(false);
  const [delayDrafts, setDelayDrafts] = useWindowState<DelayDrafts>(`profile-${scopeKey}-output-delay-drafts`, {});
  const delayDraftsRef = useRef(delayDrafts);
  delayDraftsRef.current = delayDrafts;
  const delayDirtyRef = useRef(new Set(Object.keys(delayDrafts) as DelayKey[]));
  const mountedRef = useRef(true);
  const osType = getOsType() as OSType;
  const effective = <K extends keyof OutputSettings>(key: K): OutputSettings[K] => values[key] ?? defaults[key];
  const method = effective("paste_method");
  const usesClipboard = ["ctrl_v", "ctrl_shift_v", "shift_insert"].includes(method ?? "");
  const openingKey = effective("before_insert_key") ?? "";
  const submitEnabled = effective("auto_submit") ?? false;
  const startKey = effective("recording_start_key") ?? "";
  const cancelKey = effective("cancel_key_after_early_open") ?? "";
  const earlyOn = method !== "none" && startKey !== "";
  const keyActionsBuffer = buffersOutputForKeyActions(method ?? "ctrl_v", openingKey, startKey, submitEnabled);
  const provider = getOutputProviderInfo(settings, {
    sttSelection, profilePreviewOutputOnlyEnabled: previewOutputOnly,
  }, localModels);
  const showLiveWarning = keyActionsBuffer && provider.streamsLiveText;
  const delayApplicable = (key: DelayKey): boolean => key === "paste_delay_ms" ? usesClipboard
    : key === "live_window_resume_delay_ms" ? method !== "none" && Boolean(effective("check_active_window"))
    : key === "recording_start_delay_ms" ? method !== "none" && startKey !== ""
    : method !== "none" && (key === "before_insert_delay_ms" ? openingKey !== "" : submitEnabled);
  const hasInvalidApplicableDelays = (drafts: DelayDrafts): boolean => (Object.keys(drafts) as DelayKey[])
    .some(key => delayApplicable(key) && drafts[key] != null && isDelayDraftInvalid(drafts[key]!));
  const invalidApplicableDelays = hasInvalidApplicableDelays(delayDrafts);
  useEffect(() => {
    mountedRef.current = true;
    return () => { mountedRef.current = false; };
  }, []);
  useEffect(() => {
    if (disabled || method === "none") {
      setCapturing(false);
      setCustomKeyRequested(false);
      setStartCapturing(false);
      setStartCustomRequested(false);
      setCancelCapturing(false);
      setCancelCustomRequested(false);
    }
  }, [disabled, method]);
  useEffect(() => {
    onValidationChange?.(invalidApplicableDelays);
  }, [invalidApplicableDelays, onValidationChange]);
  useEffect(() => {
    for (const key of Object.keys(delayDrafts) as DelayKey[]) {
      const draft = delayDrafts[key];
      if (!delayApplicable(key) || !draft || !isDelayDraftInvalid(draft)) continue;
      let element = document.getElementById(`profile-output-${scopeKey}-${key}`)?.parentElement;
      while (element) {
        if (element instanceof HTMLDetailsElement) element.open = true;
        element = element.parentElement;
      }
    }
  }, [delayDrafts, scopeKey, usesClipboard, method, openingKey, startKey, submitEnabled, values.check_active_window, defaults.check_active_window]);
  const updateDelayDraft = (key: DelayKey, draft?: DelayDraft) => {
    const next = { ...delayDraftsRef.current };
    if (draft) {
      next[key] = draft;
      delayDirtyRef.current.add(key);
    } else {
      delete next[key];
      delayDirtyRef.current.delete(key);
    }
    delayDraftsRef.current = next;
    setDelayDrafts(next);
    onValidationChange?.(hasInvalidApplicableDelays(next));
  };
  // An empty field in another profile stays unset (the program default) instead of saving it.
  const delayValue = (key: DelayKey, raw: string): number | null => raw.trim() === ""
    ? (isDefault ? defaults[key] ?? null : null) : Math.round(Number(raw));
  const delayInvalid = (key: DelayKey): boolean => {
    const draft = delayDrafts[key];
    return draft != null && isDelayDraftInvalid(draft);
  };
  const busy = disabled || capturing || startCapturing || cancelCapturing;
  const selected = (key: keyof OutputSettings): string => String(effective(key));
  const save = async (patch: Partial<OutputSettings>) => {
    if (disabled || !mountedRef.current) return false;
    try {
      await onChange(patch);
      return true;
    } catch (error) {
      toast.error(t("profileOutput.saveFailed"), { description: String(error) });
      return false;
    }
  };
  const reset = async () => {
    if (!await save(copyDefaultProfileOutput(defaultProfile)) || !mountedRef.current) return;
    delayDirtyRef.current.clear();
    delayDraftsRef.current = {};
    setDelayDrafts({});
    onValidationChange?.(false);
  };
  const pasteChoices: DropdownOption[] = [
    { value: "ctrl_v", label: t("settings.advanced.pasteMethod.options.clipboard", { modifier: osType === "macos" ? "Cmd" : "Ctrl" }) },
    { value: "direct", label: t("settings.advanced.pasteMethod.options.direct") },
    { value: "none", label: t("settings.advanced.pasteMethod.options.none") },
    ...(osType === "macos" ? [] : [
      { value: "ctrl_shift_v", label: t("settings.advanced.pasteMethod.options.clipboardCtrlShiftV") },
      { value: "shift_insert", label: t("settings.advanced.pasteMethod.options.clipboardShiftInsert") },
    ]),
  ];
  const clipboardChoices: DropdownOption[] = [
    { value: "keep_transcription", label: t("settings.advanced.clipboardHandling.options.keepTranscription") },
    { value: "restore_plain_text", label: t("settings.advanced.clipboardHandling.restoreMethod.options.plainText") },
    ...(osType === "windows" ? [
      { value: "restore_advanced", label: t("settings.advanced.clipboardHandling.restoreMethod.options.allFormats") },
      { value: "restore_advanced_owned", label: t("settings.advanced.clipboardHandling.restoreMethod.options.allFormatsOwned") },
    ] : []),
  ];
  const submitChoices: DropdownOption[] = [
    { value: "off", label: t("profileOutput.noKey") },
    { value: "enter", label: "Enter" },
    { value: "ctrl_enter", label: "Ctrl+Enter" },
    { value: "cmd_enter", label: osType === "macos" ? "Cmd+Enter" : "Super+Enter" },
  ];
  const choiceLabel = (choices: DropdownOption[], value: unknown) => choices.find(choice => choice.value === value)?.label ?? String(value);
  const booleanChoices = [
    { value: "true", label: t("profileOutput.enabled") },
    { value: "false", label: t("profileOutput.disabled") },
  ];
  const delay = (key: DelayKey, label: string, description = "") => (
    <SettingContainer id={`profile-output-${scopeKey}-${key}`} title={label} description={description} descriptionMode={description ? "tooltip" : "none"} tooltipWide={key === "live_window_resume_delay_ms"} grouped={true} compact={true}>
      <div className="flex min-w-0 max-w-full flex-col items-end gap-1">
      <Input
        type="number"
        min={0}
        max={5000}
        step={10}
        className="w-32 max-w-full"
        value={delayDirtyRef.current.has(key) ? delayDrafts[key]?.raw ?? "" : values[key] ?? ""}
        placeholder={String(defaults[key])}
        disabled={busy}
        aria-label={label}
        aria-invalid={delayInvalid(key)}
        aria-describedby={delayInvalid(key) ? `profile-output-${scopeKey}-${key}-error` : undefined}
        onChange={(event) => {
          if (busy) return;
          const draft = { raw: event.currentTarget.value, badInput: event.currentTarget.validity.badInput };
          updateDelayDraft(key, draft);
          if (!isDelayDraftInvalid(draft)) {
            onDraftChange?.({ [key]: delayValue(key, draft.raw) });
          }
        }}
        onBlur={(event) => {
          if (busy) return;
          const draft = delayDraftsRef.current[key] ?? { raw: event.currentTarget.value, badInput: event.currentTarget.validity.badInput };
          if (isDelayDraftInvalid(draft)) {
            if (!onDraftChange) updateDelayDraft(key);
            return;
          }
          const number = delayValue(key, draft.raw);
          updateDelayDraft(key, { raw: String(number ?? ""), badInput: false });
          if (number !== (values[key] ?? null)) {
            void save({ [key]: number }).then((saved) => {
              if (saved && mountedRef.current && delayDraftsRef.current[key]?.raw === String(number ?? "")) {
                updateDelayDraft(key);
              }
            });
          } else updateDelayDraft(key);
        }}
      />
      {delayInvalid(key) && <p id={`profile-output-${scopeKey}-${key}-error`} className="text-xs text-red-400" role="alert">
        {t("profileOutput.delayInvalid", "Enter a number from 0 to 5000 ms.")}
      </p>}
      </div>
    </SettingContainer>
  );
  const submitValue = submitEnabled ? effective("auto_submit_key") ?? "enter" : "off";
  const openingValue = customKeyRequested ? "custom"
    : openingKey === "" ? "off" : openingKey === "enter" ? "enter" : "custom";
  const startValue = startCustomRequested ? "custom"
    : startKey === "" ? "off" : startKey === "enter" ? "enter" : "custom";
  const cancelValue = cancelCustomRequested ? "custom"
    : cancelKey === "" ? "off" : cancelKey === "escape" ? "escape" : "custom";
  const waitLimitChoices: DropdownOption[] = [0, 60, 120, 300, 600].map(secs => ({ value: String(secs), label: t(`profileOutput.waitLimits.s${secs}`) }));
  const waitLimitSecs = effective("live_window_wait_limit_secs") ?? 0;
  const waitLimitAction = effective("live_window_wait_limit_action") ?? "preview";
  // A value saved outside the presets (settings file edited by hand) still shows up.
  if (!waitLimitChoices.some(choice => choice.value === String(waitLimitSecs))) {
    waitLimitChoices.push({ value: String(waitLimitSecs), label: t("profileOutput.waitLimits.custom", { seconds: waitLimitSecs }) });
  }
  const waitsForWindow = effective("live_window_stop_action") === "wait_for_window";
  const focusAtStartToggle = () => {
    const row = document.getElementById(`profile-output-${scopeKey}-${openingKey !== "" ? "before" : "open-at-start"}`);
    if (row) scrollAndFocusAnchor(row, "center");
  };
  const planInput = {
    isWindows: osType === "windows",
    heldInPreview: provider.heldInPreview,
    streamsLiveText: provider.streamsLiveText,
    pasteMethod: method ?? "ctrl_v",
    clipboardHandling: effective("clipboard_handling") ?? "restore_plain_text",
    checkActiveWindow: Boolean(effective("check_active_window")),
    resumeDelayMs: effective("live_window_resume_delay_ms") ?? 500,
    stopAction: effective("live_window_stop_action") ?? "preview",
    waitLimitSecs,
    waitLimitAction,
    openingKey,
    startKey,
    startDelayMs: effective("recording_start_delay_ms") ?? 150,
    openDelayMs: effective("before_insert_delay_ms") ?? 150,
    cancelKey,
    autoSubmit: submitEnabled,
    submitKey: effective("auto_submit_key") ?? "enter",
    submitDelayMs: effective("auto_submit_delay_ms") ?? 50,
    labels: {
      method: choiceLabel(pasteChoices, method),
      clipboard: choiceLabel(clipboardChoices, effective("clipboard_handling")),
      openingKey: formatKeyCombination(openingKey, osType),
      startKey: formatKeyCombination(startKey, osType),
      submitKey: choiceLabel(submitChoices, effective("auto_submit_key") ?? "enter"),
      cancelKey: formatKeyCombination(cancelKey, osType),
      waitLimit: choiceLabel(waitLimitChoices, String(waitLimitSecs)),
    },
  } satisfies Parameters<typeof ProfileOutputPlanPanel>[0]["input"];

  const openingKeyPicker = (atRecordingStart: boolean) => {
    const field = atRecordingStart ? "recording_start_key" : "before_insert_key";
    const key = atRecordingStart ? startKey : openingKey;
    const selectedValue = atRecordingStart ? startValue : openingValue;
    const isCapturing = atRecordingStart ? startCapturing : capturing;
    const setCapture = atRecordingStart ? setStartCapturing : setCapturing;
    const setCustom = atRecordingStart ? setStartCustomRequested : setCustomKeyRequested;
    const anchor = atRecordingStart ? "open-at-start" : "before";
    return <>
      <SettingContainer id={`profile-output-${scopeKey}-${anchor}`} title={t(atRecordingStart ? "profileOutput.beforeAtStart" : "profileOutput.before")} description={t(atRecordingStart ? "profileOutput.beforeAtStartHelp" : "profileOutput.beforeHelp")} descriptionMode="tooltip" grouped={true}>
        <Dropdown className="w-full max-w-full md:w-[200px] [&>button]:min-w-0" options={[
          { value: "off", label: t("profileOutput.noKey") },
          { value: "enter", label: "Enter" },
          { value: "custom", label: t("profileOutput.customKey") },
        ]} selectedValue={selectedValue} disabled={busy || method === "none"} onSelect={value => {
          setCustom(value === "custom");
          if (value === "custom") { setCapture(true); return; }
          void save({ [field]: value === "off" ? "" : "enter" });
        }} />
      </SettingContainer>
      {(selectedValue === "custom" || isCapturing || key !== "") && <OutputDependencies>
        {(selectedValue === "custom" || isCapturing) && <SettingContainer id={`profile-output-${scopeKey}-${atRecordingStart ? "start-key" : "opening-key"}`} title={t(atRecordingStart ? "profileOutput.startKeyPicker" : "profileOutput.openingKey")} description="" grouped={true} compact={true}>
          <HotkeyCapture value={key} isCapturing={isCapturing} osType={osType} disabled={disabled}
            onStartCapture={() => setCapture(true)} onCancel={() => { setCapture(false); setCustom(false); }}
            onCaptured={async capturedKey => {
              if (disabled || !mountedRef.current) return;
              if (!isSupportedOutputKey(capturedKey, osType)) {
                toast.error(t("profileOutput.unsupportedKey", { key: formatKeyCombination(capturedKey, osType) }));
                return;
              }
              setCapture(false); setCustom(false); await save({ [field]: capturedKey });
            }} />
        </SettingContainer>}
        {key !== "" && delay(atRecordingStart ? "recording_start_delay_ms" : "before_insert_delay_ms", t(atRecordingStart ? "profileOutput.startDelay" : "profileOutput.openDelay"))}
      </OutputDependencies>}
    </>;
  };

  return (
    <>
    <WindowDetails stateKey={`profile-${scopeKey}-output`} id={`profile-output-${scopeKey}`} tabIndex={-1} className="group rounded-lg border border-mid-gray/20 bg-mid-gray/5 overflow-hidden transition-colors open:border-purple-500/30 open:bg-purple-500/5">
      <summary className="flex items-center justify-between gap-2 px-3 py-2.5 cursor-pointer select-none list-none [&::-webkit-details-marker]:hidden hover:bg-mid-gray/10 transition-colors">
        <div className="flex items-center gap-2 min-w-0">
          <ChevronDown className="w-3.5 h-3.5 text-mid-gray shrink-0 transition-transform group-open:rotate-180" />
          <div className="min-w-0">
            <span className="block text-xs font-semibold text-text/70">{t("profileOutput.title")}</span>
            <span className="block text-[11px] text-mid-gray">{t("profileOutput.titleHint")}</span>
          </div>
        </div>
      </summary>
      <div className="border-t border-mid-gray/15">
        <p className="px-3 pt-3 text-xs text-mid-gray">
          {isDefault ? t("profileOutput.defaultHelp") : <>
            {t("profileOutput.profileHelp")}{" "}
            <button type="button" className="underline underline-offset-2 hover:text-text" onClick={openDefaultProfileOutputSettings}>{t("profileOutput.openDefault")}</button>
          </>}
        </p>
        <div id={`profile-output-${scopeKey}-more`} tabIndex={-1} className="min-w-0 pb-1">
          <SectionHeading first hint={t("profileOutput.sectionHints.insertion")}>{t("profileOutput.sections.insertion")}</SectionHeading>
          <div className="px-6 pt-4">
            <TellMeMore title={t("settings.advanced.pasteMethod.tellMeMore.title")}>
              <div className="space-y-3">
                <p className="mb-2"><strong>{t("settings.advanced.pasteMethod.tellMeMore.headline")}</strong></p>
                <p className="mb-2">{t("settings.advanced.pasteMethod.tellMeMore.intro")}</p>
                <div className="space-y-2 ml-2">
                  {["ctrlV", "direct", "none", "pasteDelay"].map((key) => (
                    <p key={key}>
                      <strong>{t(`settings.advanced.pasteMethod.tellMeMore.${key}.title`)}</strong>{" "}
                      {t(`settings.advanced.pasteMethod.tellMeMore.${key}.description`)}
                    </p>
                  ))}
                </div>
                <p className="mt-3 text-text/70 text-xs">{t("settings.advanced.pasteMethod.tellMeMore.tip")}</p>
              </div>
            </TellMeMore>
          </div>
          <SettingContainer id={`profile-output-${scopeKey}-paste-method`} title={t("settings.advanced.pasteMethod.title")} description={t("settings.advanced.pasteMethod.description")} descriptionMode="tooltip" grouped={true}>
            <Dropdown className="w-full max-w-full md:w-[200px] [&>button]:min-w-0" options={pasteChoices} selectedValue={selected("paste_method")} disabled={busy} onSelect={value => void save({ paste_method: value as OutputSettings["paste_method"] })} />
          </SettingContainer>
          {method !== "none" && usesClipboard && osType === "windows" && <OutputDependencies>
                <SettingContainer id={`profile-output-${scopeKey}-line-endings`} title={t("settings.advanced.pasteMethod.convertLfToCrlf.label")} description={t("settings.advanced.pasteMethod.convertLfToCrlf.description")} descriptionMode="tooltip" grouped={true} compact={true}>
                  <Dropdown className="w-full max-w-full md:w-[200px] [&>button]:min-w-0" options={booleanChoices} selectedValue={selected("convert_lf_to_crlf")} disabled={busy} onSelect={value => void save({ convert_lf_to_crlf: value === "true" })} />
                </SettingContainer>
          </OutputDependencies>}
          {method !== "none" && usesClipboard && <div id={`profile-output-${scopeKey}-clipboard-panel`} tabIndex={-1} className="min-w-0">
            <SectionHeading hint={t("profileOutput.sectionHints.clipboard")}>{t("profileOutput.sections.clipboard")}</SectionHeading>
              <div className="px-6 pt-4">
                <TellMeMore title={t("settings.advanced.tellMeMore.clipboardHandling.title")}>
                  <div className="space-y-3 text-text/90">
                    <p>{t("settings.advanced.tellMeMore.clipboardHandling.intro")}</p>
                    <p className="text-sm">
                      <strong>{t("settings.advanced.tellMeMore.clipboardHandling.safestChoiceTitle")}</strong>{" "}
                      {t("settings.advanced.tellMeMore.clipboardHandling.safestChoiceDescription")}
                    </p>
                    <div className="space-y-2 ml-2">
                      {["keepTranscription", "restore", "restorePlain", "restoreAdvanced", "restoreAdvancedOwned", "history"].map((key) => (
                        <p key={key}>
                          <strong>{t(`settings.advanced.tellMeMore.clipboardHandling.${key}.title`)}</strong>{" "}
                          {t(`settings.advanced.tellMeMore.clipboardHandling.${key}.description`)}
                        </p>
                      ))}
                    </div>
                    <p className="text-xs text-text/70">{t("settings.advanced.tellMeMore.clipboardHandling.note")}</p>
                  </div>
                </TellMeMore>
              </div>
              <SettingContainer id={`profile-output-${scopeKey}-clipboard`} title={t("profileOutput.clipboard")} description={t("settings.advanced.clipboardHandling.description")} descriptionMode="tooltip" grouped={true} compact={true}>
                <Dropdown className="w-full max-w-full md:w-[200px] [&>button]:min-w-0" options={clipboardChoices} selectedValue={selected("clipboard_handling")} disabled={busy} onSelect={value => void save({ clipboard_handling: value as OutputSettings["clipboard_handling"] })} />
              </SettingContainer>
        {osType === "windows" && <OutputDependencies nested>
                {effective("clipboard_handling") !== "keep_transcription" && <SettingContainer id={`profile-output-${scopeKey}-clipboard-history`} title={t("settings.advanced.clipboardHandling.history.title")} description={t("settings.advanced.clipboardHandling.history.description")} descriptionMode="tooltip" grouped={true} compact={true}>
                  <Dropdown className="w-full max-w-full md:w-[200px] [&>button]:min-w-0" options={booleanChoices} selectedValue={selected("clipboard_history_allowed")} disabled={busy} onSelect={value => void save({ clipboard_history_allowed: value === "true" })} />
                </SettingContainer>}
        </OutputDependencies>}
        {delay("paste_delay_ms", t("profileOutput.pasteDelay"))}
          </div>}
          {method !== "none" && osType === "windows" && <>
            <SectionHeading hint={t("profileOutput.sectionHints.window")}>{t("profileOutput.sections.window")}</SectionHeading>
            {osType === "windows" && <SettingContainer id={`profile-output-${scopeKey}-check_active_window`} title={t("profileOutput.checkActiveWindow")} description={t("profileOutput.checkActiveWindowHelp")} descriptionMode="tooltip" tooltipWide grouped={true} compact={true}>
              <Dropdown className="w-full max-w-full md:w-[200px] [&>button]:min-w-0" options={booleanChoices} selectedValue={selected("check_active_window")} disabled={busy} onSelect={value => void save({ check_active_window: value === "true" })} />
            </SettingContainer>}
            {effective("check_active_window") && <OutputDependencies>
              {delay("live_window_resume_delay_ms", t("profileOutput.liveWindowResumeDelay"), t("profileOutput.liveWindowResumeDelayHelp"))}
              <SettingContainer id={`profile-output-${scopeKey}-live_window_stop_action`} title={t("profileOutput.liveWindowStopAction")} description={t("profileOutput.liveWindowStopActionHelp")} descriptionMode="tooltip" grouped={true} compact={true} layout="stacked">
                <div className="space-y-2" role="radiogroup" aria-label={t("profileOutput.liveWindowStopAction")}>
                  {(["history", "preview", "wait_for_window"] as const).map(action => (
                    <label key={action} className="flex items-start gap-2 cursor-pointer text-xs text-text/80">
                      <input type="radio" name={`profile-live-window-stop-${scopeKey}`} value={action}
                        checked={effective("live_window_stop_action") === action} disabled={busy}
                        className="mt-0.5 accent-purple-500"
                        onChange={() => void save({ live_window_stop_action: action })} />
                      <span>{t(`profileOutput.liveWindowStopActions.${action}`)}</span>
                    </label>
                  ))}
                </div>
              </SettingContainer>
              {waitsForWindow && <SettingContainer id={`profile-output-${scopeKey}-live_window_wait_limit_secs`} title={t("profileOutput.waitLimit")} description={t("profileOutput.waitLimitHelp")} descriptionMode="tooltip" grouped={true} compact={true}>
                <Dropdown className="w-full max-w-full md:w-[200px] [&>button]:min-w-0" options={waitLimitChoices} selectedValue={selected("live_window_wait_limit_secs")} disabled={busy} onSelect={value => void save({ live_window_wait_limit_secs: Number(value) })} />
              </SettingContainer>}
              {waitsForWindow && waitLimitSecs > 0 && <OutputDependencies nested>
                <SettingContainer id={`profile-output-${scopeKey}-live_window_wait_limit_action`} title={t("profileOutput.waitLimitAction")} description="" grouped={true} compact={true} layout="stacked">
                  <div className="space-y-2" role="radiogroup" aria-label={t("profileOutput.waitLimitAction")}>
                    {(["preview", "history"] as const).map(action => (
                      <label key={action} className="flex items-start gap-2 cursor-pointer text-xs text-text/80">
                        <input type="radio" name={`profile-live-window-wait-limit-${scopeKey}`} value={action}
                          checked={waitLimitAction === action} disabled={busy}
                          className="mt-0.5 accent-purple-500"
                          onChange={() => void save({ live_window_wait_limit_action: action })} />
                        <span>{t(`profileOutput.waitLimitActions.${action}`)}</span>
                      </label>
                    ))}
                  </div>
                </SettingContainer>
              </OutputDependencies>}
            </OutputDependencies>}
          </>}
        </div>
      </div>
    </WindowDetails>
    {method !== "none" && <WindowDetails stateKey={`profile-${scopeKey}-output-keys`} id={`profile-output-${scopeKey}-keys-panel`} tabIndex={-1} className="group rounded-lg border border-mid-gray/20 bg-mid-gray/5 overflow-hidden transition-colors open:border-purple-500/30 open:bg-purple-500/5">
      <summary className="flex items-center justify-between gap-2 px-3 py-2.5 cursor-pointer select-none list-none [&::-webkit-details-marker]:hidden hover:bg-mid-gray/10 transition-colors">
        <div className="flex items-center gap-2 min-w-0">
          <ChevronDown className="w-3.5 h-3.5 text-mid-gray shrink-0 transition-transform group-open:rotate-180" />
          <div className="min-w-0">
            <span className="block text-xs font-semibold text-text/70">{t("profileOutput.keysPanel.title")}</span>
            <span className="block text-[11px] text-mid-gray">{t("profileOutput.keysPanel.hint")}</span>
          </div>
        </div>
        {(submitEnabled || openingKey !== "" || startKey !== "") && <KeyActionsBadge />}
      </summary>
      <div className="min-w-0 border-t border-mid-gray/15 pb-1">
        <p className="mx-6 mt-3 mb-1 text-[11px] text-amber-300/80">
          <Trans i18nKey="profileOutput.keysPanel.standardOnly" components={{
            repaste: <button type="button" className="underline underline-offset-2 hover:text-amber-200" onClick={openRepasteShortcut} />,
          }} />
        </p>
        <p className="mx-6 mt-3 mb-1 text-xs text-mid-gray">{t("profileOutput.keysPanel.beforeHint")}</p>
            {openingKeyPicker(true)}
            {openingKeyPicker(false)}
            <p className="mx-6 mt-3 mb-1 text-xs text-mid-gray">{t("profileOutput.keysPanel.afterHint")}</p>
            <SettingContainer id={`profile-output-${scopeKey}-after`} title={t("profileOutput.after")} description={t("settings.advanced.autoSubmit.description")} descriptionMode="tooltip" grouped={true}>
              <Dropdown className="w-full max-w-full md:w-[200px] [&>button]:min-w-0" options={submitChoices} selectedValue={submitValue} disabled={busy} onSelect={value => void save(value === "off" ? { auto_submit: false } : { auto_submit: true, auto_submit_key: value as OutputSettings["auto_submit_key"] })} />
            </SettingContainer>
            {submitEnabled && <OutputDependencies>
              {delay("auto_submit_delay_ms", t("profileOutput.submitDelay"))}
            </OutputDependencies>}
            {earlyOn && <>
              <SettingContainer id={`profile-output-${scopeKey}-cancel-key`} title={t("profileOutput.cancelKey")} description={t("profileOutput.cancelKeyHelp")} descriptionMode="tooltip" grouped={true}>
                <Dropdown className="w-full max-w-full md:w-[200px] [&>button]:min-w-0" options={[
                  { value: "off", label: t("profileOutput.noKey") },
                  { value: "escape", label: "Escape" },
                  { value: "custom", label: t("profileOutput.customKey") },
                ]} selectedValue={cancelValue} disabled={busy} onSelect={value => {
                  setCancelCustomRequested(value === "custom");
                  if (value === "custom") { setCancelCapturing(true); return; }
                  void save({ cancel_key_after_early_open: value === "off" ? "" : "escape" });
                }} />
              </SettingContainer>
              {(cancelValue === "custom" || cancelCapturing) && <OutputDependencies>
                <SettingContainer id={`profile-output-${scopeKey}-cancel-key-capture`} title={t("profileOutput.cancelKeyPicker")} description="" grouped={true} compact={true}>
                  <HotkeyCapture value={cancelKey} isCapturing={cancelCapturing} osType={osType} disabled={disabled}
                    onStartCapture={() => setCancelCapturing(true)} onCancel={() => { setCancelCapturing(false); setCancelCustomRequested(false); }}
                    onCaptured={async key => {
                      if (disabled || !mountedRef.current) return;
                      if (!isSupportedOutputKey(key, osType)) {
                        toast.error(t("profileOutput.unsupportedKey", { key: formatKeyCombination(key, osType) }));
                        return;
                      }
                      setCancelCapturing(false); setCancelCustomRequested(false); await save({ cancel_key_after_early_open: key });
                    }} />
                </SettingContainer>
              </OutputDependencies>}
            </>}
            {(openingKey !== "" || startKey !== "" || submitEnabled) && (showLiveWarning
            ? <div className="mx-6 mb-3 flex items-start gap-2 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2">
                <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-red-400" aria-hidden="true" />
                <p className="text-xs text-red-300/90">
                  <Trans i18nKey={openingKey !== "" ? "profileOutput.liveBufferedWarning" : "profileOutput.liveBufferedWarningNoOpeningKey"} components={{
                        toggle: <button type="button" className="underline underline-offset-2 hover:text-red-200" onClick={focusAtStartToggle} />,
                      }} />
                </p>
              </div>
            : <p className="px-6 pb-3 text-xs text-mid-gray">{t(earlyOn ? "profileOutput.earlyOpenHelp" : "profileOutput.bufferedHelp")}</p>)}
      </div>
    </WindowDetails>}
    <ProfileOutputPlanPanel scopeKey={scopeKey} input={planInput} />
    {!isDefault && <div className="px-1">
      <button type="button" disabled={busy} className="py-2 text-xs text-mid-gray underline underline-offset-2 hover:text-text disabled:opacity-50" onClick={() => void reset()}>{t("profileOutput.reset")}</button>
    </div>}
    </>
  );
};
