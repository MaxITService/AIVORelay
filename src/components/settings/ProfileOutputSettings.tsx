import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { type as getOsType } from "@tauri-apps/plugin-os";
import { invoke } from "@tauri-apps/api/core";
import { ChevronDown } from "lucide-react";
import { Dropdown, type DropdownOption } from "../ui/Dropdown";
import { HotkeyCapture } from "../ui/HotkeyCapture";
import { Input } from "../ui/Input";
import { SettingContainer } from "../ui/SettingContainer";
import { useSettings } from "../../hooks/useSettings";
import { sessionToast as toast } from "@/lib/sessionToast";
import { formatKeyCombination, type OSType } from "@/lib/utils/keyboard";
import { isSupportedOutputKey, type ProfileOutputSettings as OutputSettings } from "@/lib/profileOutputSettings";
import { openDefaultProfileOutputSettings } from "@/lib/transcriptionProfileNavigation";

interface Props {
  profileId: string;
  overrides?: OutputSettings;
  disabled?: boolean;
}

export const ProfileOutputSettings: React.FC<Props> = ({ profileId, overrides = {}, disabled = false }) => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();
  const [saving, setSaving] = useState(false);
  const [capturing, setCapturing] = useState(false);
  const [customKeyRequested, setCustomKeyRequested] = useState(false);
  const osType = getOsType() as OSType;
  const isDefault = profileId === "default";
  const defaults: OutputSettings = {
    paste_method: settings?.paste_method ?? "ctrl_v",
    clipboard_handling: settings?.clipboard_handling ?? "restore_plain_text",
    clipboard_history_allowed: settings?.clipboard_history_allowed ?? true,
    convert_lf_to_crlf: settings?.convert_lf_to_crlf ?? true,
    paste_delay_ms: settings?.paste_delay_ms ?? 60,
    auto_submit: settings?.auto_submit ?? false,
    auto_submit_key: settings?.auto_submit_key ?? "enter",
    before_insert_key: settings?.transcription_output_defaults?.before_insert_key ?? "",
    before_insert_delay_ms: settings?.transcription_output_defaults?.before_insert_delay_ms ?? 150,
    auto_submit_delay_ms: settings?.transcription_output_defaults?.auto_submit_delay_ms ?? 50,
  };
  const values = isDefault ? defaults : overrides;
  const effective = <K extends keyof OutputSettings>(key: K): OutputSettings[K] => values[key] ?? defaults[key];
  const busy = disabled || saving || capturing;
  const method = effective("paste_method");
  const usesClipboard = ["ctrl_v", "ctrl_shift_v", "shift_insert"].includes(method ?? "");
  const openingKey = effective("before_insert_key") ?? "";
  const submitEnabled = effective("auto_submit") ?? false;
  const inherited = (key: keyof OutputSettings) => !isDefault && values[key] == null;
  const ownValue = (key: keyof OutputSettings): string => inherited(key) ? "inherit" : String(effective(key));
  const options = (choices: DropdownOption[], defaultLabel: string): DropdownOption[] => isDefault ? choices : [
    { value: "inherit", label: t("profileOutput.inherit", { value: defaultLabel }) }, ...choices,
  ];
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
  const delay = (key: "paste_delay_ms" | "before_insert_delay_ms" | "auto_submit_delay_ms", label: string) => (
    <SettingContainer title={label} description="" grouped={true}>
      <Input
        key={`${profileId}-${key}-${values[key] ?? "inherit"}`}
        type="number"
        min={0}
        max={5000}
        step={10}
        className="w-32"
        defaultValue={values[key] ?? ""}
        placeholder={String(defaults[key])}
        disabled={busy}
        aria-label={label}
        onBlur={(event) => {
          const raw = event.currentTarget.value.trim();
          const number = raw === "" ? (isDefault ? defaults[key] : null) : Math.round(Number(raw));
          if (number != null && (!Number.isFinite(number) || number < 0 || number > 5000)) {
            event.currentTarget.value = String(values[key] ?? "");
            return;
          }
          if (number !== (values[key] ?? null)) {
            const patch: Partial<OutputSettings> = {};
            patch[key] = number;
            void save(patch);
          }
        }}
      />
    </SettingContainer>
  );
  const submitValue = !isDefault && values.auto_submit == null && values.auto_submit_key == null
    ? "inherit" : submitEnabled ? effective("auto_submit_key") ?? "enter" : "off";
  const openingValue = customKeyRequested ? "custom" : inherited("before_insert_key")
    ? "inherit" : openingKey === "" ? "off" : openingKey === "enter" ? "enter" : "custom";

  return (
    <details id={`profile-output-${profileId}`} tabIndex={-1} className="group rounded-lg border border-mid-gray/20 bg-mid-gray/5 overflow-hidden transition-colors open:border-purple-500/30 open:bg-purple-500/5">
      <summary className="flex items-center justify-between gap-2 px-3 py-2.5 cursor-pointer select-none list-none [&::-webkit-details-marker]:hidden hover:bg-mid-gray/10 transition-colors">
        <div className="flex items-center gap-2 min-w-0">
          <ChevronDown className="w-3.5 h-3.5 text-mid-gray shrink-0 transition-transform group-open:rotate-180" />
          <span className="text-xs font-semibold text-text/70">{t("profileOutput.title")}</span>
        </div>
        {(submitEnabled || openingKey !== "") && <span className="text-[10px] text-purple-400">{t("profileOutput.active")}</span>}
      </summary>
      <div className="border-t border-mid-gray/15">
        <p className="px-3 pt-3 text-xs text-mid-gray">
          {isDefault ? t("profileOutput.defaultHelp") : <>
            {t("profileOutput.profileHelp")}{" "}
            <button type="button" className="underline underline-offset-2 hover:text-text" onClick={openDefaultProfileOutputSettings}>{t("profileOutput.openDefault")}</button>
          </>}
        </p>
        <SettingContainer title={t("settings.advanced.pasteMethod.title")} description={t("settings.advanced.pasteMethod.description")} descriptionMode="tooltip" grouped={true}>
          <Dropdown options={options(pasteChoices, choiceLabel(pasteChoices, defaults.paste_method))} selectedValue={ownValue("paste_method")} disabled={busy} onSelect={value => void save({ paste_method: value === "inherit" ? null : value as OutputSettings["paste_method"] })} />
        </SettingContainer>
        {usesClipboard && <SettingContainer title={t("profileOutput.clipboard")} description={t("settings.advanced.clipboardHandling.description")} descriptionMode="tooltip" grouped={true}>
          <Dropdown options={options(clipboardChoices, choiceLabel(clipboardChoices, defaults.clipboard_handling))} selectedValue={ownValue("clipboard_handling")} disabled={busy} onSelect={value => void save({ clipboard_handling: value === "inherit" ? null : value as OutputSettings["clipboard_handling"] })} />
        </SettingContainer>}
        <SettingContainer title={t("profileOutput.before")} description={t("profileOutput.beforeHelp")} descriptionMode="tooltip" grouped={true}>
          <Dropdown options={options([
            { value: "off", label: t("profileOutput.noKey") },
            { value: "enter", label: "Enter" },
            { value: "custom", label: t("profileOutput.customKey") },
          ], defaults.before_insert_key || t("profileOutput.noKey"))} selectedValue={openingValue} disabled={busy || method === "none"} onSelect={value => {
            setCustomKeyRequested(value === "custom");
            if (value === "custom") { setCapturing(true); return; }
            void save({ before_insert_key: value === "inherit" ? null : value === "off" ? "" : "enter" });
          }} />
        </SettingContainer>
        {(openingValue === "custom" || capturing) && <SettingContainer title={t("profileOutput.openingKey")} description="" grouped={true}>
          <HotkeyCapture value={openingKey} isCapturing={capturing} osType={osType} disabled={saving || disabled}
            onStartCapture={() => setCapturing(true)} onCancel={() => { setCapturing(false); setCustomKeyRequested(false); }}
            onCaptured={async key => {
              if (!isSupportedOutputKey(key, osType)) {
                toast.error(t("profileOutput.unsupportedKey", { key: formatKeyCombination(key, osType) }));
                return;
              }
              setCapturing(false); setCustomKeyRequested(false); await save({ before_insert_key: key });
            }} />
        </SettingContainer>}
        <SettingContainer title={t("profileOutput.after")} description={t("settings.advanced.autoSubmit.description")} descriptionMode="tooltip" grouped={true}>
          <Dropdown options={options(submitChoices, choiceLabel(submitChoices, defaults.auto_submit ? defaults.auto_submit_key : "off"))} selectedValue={submitValue} disabled={busy || method === "none"} onSelect={value => void save(value === "inherit"
            ? { auto_submit: null, auto_submit_key: null }
            : value === "off" ? { auto_submit: false } : { auto_submit: true, auto_submit_key: value as OutputSettings["auto_submit_key"] })} />
        </SettingContainer>
        {(openingKey !== "" || submitEnabled) && method !== "none" && <p className="px-3 pb-3 text-xs text-mid-gray">{t("profileOutput.bufferedHelp")}</p>}
        <details className="mx-3 mb-3 border-t border-mid-gray/15">
          <summary className="py-2 text-xs text-mid-gray cursor-pointer">{t("profileOutput.more")}</summary>
          {usesClipboard && delay("paste_delay_ms", t("profileOutput.pasteDelay"))}
          {openingKey !== "" && delay("before_insert_delay_ms", t("profileOutput.openDelay"))}
          {submitEnabled && delay("auto_submit_delay_ms", t("profileOutput.submitDelay"))}
          {usesClipboard && osType === "windows" && <>
            {effective("clipboard_handling") !== "keep_transcription" && <SettingContainer title={t("settings.advanced.clipboardHandling.history.title")} description={t("settings.advanced.clipboardHandling.history.description")} descriptionMode="tooltip" grouped={true}>
              <Dropdown options={options(booleanChoices, choiceLabel(booleanChoices, String(defaults.clipboard_history_allowed)))} selectedValue={ownValue("clipboard_history_allowed")} disabled={busy} onSelect={value => void save({ clipboard_history_allowed: value === "inherit" ? null : value === "true" })} />
            </SettingContainer>}
            <SettingContainer title={t("settings.advanced.pasteMethod.convertLfToCrlf.label")} description={t("settings.advanced.pasteMethod.convertLfToCrlf.description")} descriptionMode="tooltip" grouped={true}>
              <Dropdown options={options(booleanChoices, choiceLabel(booleanChoices, String(defaults.convert_lf_to_crlf)))} selectedValue={ownValue("convert_lf_to_crlf")} disabled={busy} onSelect={value => void save({ convert_lf_to_crlf: value === "inherit" ? null : value === "true" })} />
            </SettingContainer>
          </>}
          {!isDefault && <button type="button" disabled={busy} className="py-2 text-xs text-mid-gray underline underline-offset-2 hover:text-text disabled:opacity-50" onClick={() => void save(Object.fromEntries(Object.keys(overrides).map(key => [key, null])))}>{t("profileOutput.reset")}</button>}
        </details>
      </div>
    </details>
  );
};
