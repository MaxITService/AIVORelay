import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { type as getOsType } from "@tauri-apps/plugin-os";
import { invoke } from "@tauri-apps/api/core";
import { ChevronDown } from "lucide-react";
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

interface Props {
  profileId: string;
  overrides?: OutputSettings;
  disabled?: boolean;
}

export const getGlobalProfileOutputDefaults = (settings: Settings | null | undefined): OutputSettings => ({
    paste_method: settings?.paste_method ?? "ctrl_v",
    clipboard_handling: settings?.clipboard_handling ?? "restore_plain_text",
    clipboard_history_allowed: settings?.clipboard_history_allowed ?? true,
    convert_lf_to_crlf: settings?.convert_lf_to_crlf ?? true,
    paste_delay_ms: settings?.paste_delay_ms ?? 60,
    check_active_window: settings?.transcription_output_defaults?.check_active_window ?? true,
    auto_submit: settings?.auto_submit ?? false,
    auto_submit_key: settings?.auto_submit_key ?? "enter",
    before_insert_key: settings?.transcription_output_defaults?.before_insert_key ?? "",
    before_insert_delay_ms: settings?.transcription_output_defaults?.before_insert_delay_ms ?? 150,
    auto_submit_delay_ms: settings?.transcription_output_defaults?.auto_submit_delay_ms ?? 50,
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
}

type DelayKey = "paste_delay_ms" | "before_insert_delay_ms" | "auto_submit_delay_ms";
type DelayDraft = { raw: string; badInput: boolean };
type DelayDrafts = Partial<Record<DelayKey, DelayDraft>>;

const isDelayDraftInvalid = ({ raw, badInput }: DelayDraft): boolean => {
  const number = Number(raw);
  return badInput || (raw.trim() !== "" && (!Number.isFinite(number) || number < 0 || number > 5000));
};

export const ProfileOutputSettingsFields: React.FC<ProfileOutputSettingsFieldsProps> = (props) => (
  <ProfileOutputSettingsFieldsContent key={props.scopeKey} {...props} />
);

const OutputDependencies: React.FC<{ children: React.ReactNode; nested?: boolean }> = ({ children, nested = false }) => (
  <div className={`${nested ? "ml-2" : "ml-4"} mb-1 min-w-0 border-l border-mid-gray/15 [&>div[id]]:pl-4 [&>div[id]>div:last-child]:min-w-0 [&>div[id]>div:last-child]:max-w-full`}>
    {children}
  </div>
);

const ProfileOutputSettingsFieldsContent: React.FC<ProfileOutputSettingsFieldsProps> = ({
  scopeKey, values, defaults, isDefault = false, disabled = false, onChange, onDraftChange, onValidationChange,
}) => {
  const { t } = useTranslation();
  const [capturing, setCapturing] = useState(false);
  const [customKeyRequested, setCustomKeyRequested] = useState(false);
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
  const delayApplicable = (key: DelayKey): boolean => key === "paste_delay_ms" ? usesClipboard
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
  }, [delayDrafts, scopeKey, usesClipboard, method, openingKey, submitEnabled]);
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
  const delayValue = (key: DelayKey, raw: string): number | null => raw.trim() === ""
    ? (isDefault ? defaults[key] ?? null : null) : Math.round(Number(raw));
  const delayInvalid = (key: DelayKey): boolean => {
    const draft = delayDrafts[key];
    return draft != null && isDelayDraftInvalid(draft);
  };
  const busy = disabled || capturing;
  const inherited = (key: keyof OutputSettings) => !isDefault && values[key] == null;
  const ownValue = (key: keyof OutputSettings): string => inherited(key) ? "inherit" : String(effective(key));
  const options = (choices: DropdownOption[], defaultLabel: string): DropdownOption[] => isDefault ? choices : [
    { value: "inherit", label: t("profileOutput.inherit", { value: defaultLabel }) }, ...choices,
  ];
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
    if (!await save(Object.fromEntries(Object.keys(values).map(key => [key, null]))) || !mountedRef.current) return;
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
  const delay = (key: DelayKey, label: string) => (
    <SettingContainer id={`profile-output-${scopeKey}-${key}`} title={label} description="" grouped={true} compact={true}>
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
  const submitValue = !isDefault && values.auto_submit == null && values.auto_submit_key == null
    ? "inherit" : submitEnabled ? effective("auto_submit_key") ?? "enter" : "off";
  const openingValue = customKeyRequested ? "custom" : inherited("before_insert_key")
    ? "inherit" : openingKey === "" ? "off" : openingKey === "enter" ? "enter" : "custom";

  return (
    <WindowDetails stateKey={`profile-${scopeKey}-output`} id={`profile-output-${scopeKey}`} tabIndex={-1} className="group rounded-lg border border-mid-gray/20 bg-mid-gray/5 overflow-hidden transition-colors open:border-purple-500/30 open:bg-purple-500/5">
      <summary className="flex items-center justify-between gap-2 px-3 py-2.5 cursor-pointer select-none list-none [&::-webkit-details-marker]:hidden hover:bg-mid-gray/10 transition-colors">
        <div className="flex items-center gap-2 min-w-0">
          <ChevronDown className="w-3.5 h-3.5 text-mid-gray shrink-0 transition-transform group-open:rotate-180" />
          <span className="text-xs font-semibold text-text/70">{t("profileOutput.title")}</span>
        </div>
        {method !== "none" && (submitEnabled || openingKey !== "") && <span className="text-[10px] text-purple-400">{t("profileOutput.active")}</span>}
      </summary>
      <div className="border-t border-mid-gray/15">
        <p className="px-3 pt-3 text-xs text-mid-gray">
          {isDefault ? t("profileOutput.defaultHelp") : <>
            {t("profileOutput.profileHelp")}{" "}
            <button type="button" className="underline underline-offset-2 hover:text-text" onClick={openDefaultProfileOutputSettings}>{t("profileOutput.openDefault")}</button>
          </>}
        </p>
        <div id={`profile-output-${scopeKey}-more`} tabIndex={-1} className="min-w-0 pb-1">
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
            <Dropdown className="w-full max-w-full md:w-[200px] [&>button]:min-w-0" options={options(pasteChoices, choiceLabel(pasteChoices, defaults.paste_method))} selectedValue={ownValue("paste_method")} disabled={busy} onSelect={value => void save({ paste_method: value === "inherit" ? null : value as OutputSettings["paste_method"] })} />
          </SettingContainer>
          {method !== "none" && (usesClipboard || osType === "windows") && <OutputDependencies>
            {usesClipboard && delay("paste_delay_ms", t("profileOutput.pasteDelay"))}
            {usesClipboard && <>
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
                <Dropdown className="w-full max-w-full md:w-[200px] [&>button]:min-w-0" options={options(clipboardChoices, choiceLabel(clipboardChoices, defaults.clipboard_handling))} selectedValue={ownValue("clipboard_handling")} disabled={busy} onSelect={value => void save({ clipboard_handling: value === "inherit" ? null : value as OutputSettings["clipboard_handling"] })} />
              </SettingContainer>
              {osType === "windows" && <OutputDependencies nested>
                {effective("clipboard_handling") !== "keep_transcription" && <SettingContainer id={`profile-output-${scopeKey}-clipboard-history`} title={t("settings.advanced.clipboardHandling.history.title")} description={t("settings.advanced.clipboardHandling.history.description")} descriptionMode="tooltip" grouped={true} compact={true}>
                  <Dropdown className="w-full max-w-full md:w-[200px] [&>button]:min-w-0" options={options(booleanChoices, choiceLabel(booleanChoices, String(defaults.clipboard_history_allowed)))} selectedValue={ownValue("clipboard_history_allowed")} disabled={busy} onSelect={value => void save({ clipboard_history_allowed: value === "inherit" ? null : value === "true" })} />
                </SettingContainer>}
                <SettingContainer id={`profile-output-${scopeKey}-line-endings`} title={t("settings.advanced.pasteMethod.convertLfToCrlf.label")} description={t("settings.advanced.pasteMethod.convertLfToCrlf.description")} descriptionMode="tooltip" grouped={true} compact={true}>
                  <Dropdown className="w-full max-w-full md:w-[200px] [&>button]:min-w-0" options={options(booleanChoices, choiceLabel(booleanChoices, String(defaults.convert_lf_to_crlf)))} selectedValue={ownValue("convert_lf_to_crlf")} disabled={busy} onSelect={value => void save({ convert_lf_to_crlf: value === "inherit" ? null : value === "true" })} />
                </SettingContainer>
              </OutputDependencies>}
            </>}
            {osType === "windows" && <SettingContainer id={`profile-output-${scopeKey}-check_active_window`} title={t("profileOutput.checkActiveWindow")} description={t("profileOutput.checkActiveWindowHelp")} descriptionMode="tooltip" grouped={true} compact={true}>
              <Dropdown className="w-full max-w-full md:w-[200px] [&>button]:min-w-0" options={options(booleanChoices, choiceLabel(booleanChoices, String(defaults.check_active_window)))} selectedValue={ownValue("check_active_window")} disabled={busy || method === "none"} onSelect={value => void save({ check_active_window: value === "inherit" ? null : value === "true" })} />
            </SettingContainer>}
          </OutputDependencies>}
          {method !== "none" && <>
            <SettingContainer id={`profile-output-${scopeKey}-before`} title={t("profileOutput.before")} description={t("profileOutput.beforeHelp")} descriptionMode="tooltip" grouped={true}>
              <Dropdown className="w-full max-w-full md:w-[200px] [&>button]:min-w-0" options={options([
                { value: "off", label: t("profileOutput.noKey") },
                { value: "enter", label: "Enter" },
                { value: "custom", label: t("profileOutput.customKey") },
              ], defaults.before_insert_key || t("profileOutput.noKey"))} selectedValue={openingValue} disabled={busy || method === "none"} onSelect={value => {
                setCustomKeyRequested(value === "custom");
                if (value === "custom") { setCapturing(true); return; }
                void save({ before_insert_key: value === "inherit" ? null : value === "off" ? "" : "enter" });
              }} />
            </SettingContainer>
            {(openingValue === "custom" || capturing || openingKey !== "") && <OutputDependencies>
              {(openingValue === "custom" || capturing) && <SettingContainer id={`profile-output-${scopeKey}-opening-key`} title={t("profileOutput.openingKey")} description="" grouped={true} compact={true}>
                <HotkeyCapture value={openingKey} isCapturing={capturing} osType={osType} disabled={disabled}
                  onStartCapture={() => setCapturing(true)} onCancel={() => { setCapturing(false); setCustomKeyRequested(false); }}
                  onCaptured={async key => {
                    if (disabled || !mountedRef.current) return;
                    if (!isSupportedOutputKey(key, osType)) {
                      toast.error(t("profileOutput.unsupportedKey", { key: formatKeyCombination(key, osType) }));
                      return;
                    }
                    setCapturing(false); setCustomKeyRequested(false); await save({ before_insert_key: key });
                  }} />
              </SettingContainer>}
              {openingKey !== "" && delay("before_insert_delay_ms", t("profileOutput.openDelay"))}
            </OutputDependencies>}
            <SettingContainer id={`profile-output-${scopeKey}-after`} title={t("profileOutput.after")} description={t("settings.advanced.autoSubmit.description")} descriptionMode="tooltip" grouped={true}>
              <Dropdown className="w-full max-w-full md:w-[200px] [&>button]:min-w-0" options={options(submitChoices, choiceLabel(submitChoices, defaults.auto_submit ? defaults.auto_submit_key : "off"))} selectedValue={submitValue} disabled={busy || method === "none"} onSelect={value => void save(value === "inherit"
                ? { auto_submit: null, auto_submit_key: null }
                : value === "off" ? { auto_submit: false } : { auto_submit: true, auto_submit_key: value as OutputSettings["auto_submit_key"] })} />
            </SettingContainer>
            {submitEnabled && <OutputDependencies>
              {delay("auto_submit_delay_ms", t("profileOutput.submitDelay"))}
            </OutputDependencies>}
          </>}
          {(openingKey !== "" || submitEnabled) && method !== "none" && <p className="px-6 pb-3 text-xs text-mid-gray">{t("profileOutput.bufferedHelp")}</p>}
          {!isDefault && <div className="px-6 pb-3">
            <button type="button" disabled={busy} className="py-2 text-xs text-mid-gray underline underline-offset-2 hover:text-text disabled:opacity-50" onClick={() => void reset()}>{t("profileOutput.reset")}</button>
          </div>}
        </div>
      </div>
    </WindowDetails>
  );
};

export const ProfileOutputSettings: React.FC<Props> = ({ profileId, overrides = {}, disabled = false }) => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();
  const [saving, setSaving] = useState(false);
  const isDefault = profileId === "default";
  const defaults = getGlobalProfileOutputDefaults(settings);
  const values = isDefault ? defaults : overrides;
  const save = async (patch: Partial<OutputSettings>) => {
    if (saving || disabled) return;
    setSaving(true);
    try {
      await invoke("change_transcription_output_settings", {
        profileId,
        output: { ...values, ...patch },
      });
      await refreshSettings();
    } catch (error) {
      toast.error(t("profileOutput.saveFailed"), { description: String(error) });
    } finally {
      setSaving(false);
    }
  };
  return (
    <ProfileOutputSettingsFields scopeKey={profileId} values={values} defaults={defaults}
      isDefault={isDefault} disabled={disabled || saving} onChange={save} />
  );
};
