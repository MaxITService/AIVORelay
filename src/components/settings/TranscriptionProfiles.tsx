import React, { useState, useMemo, useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  Plus,
  Trash2,
  ChevronDown,
  ChevronUp,
  Globe,
  Check,
  Play,
  RefreshCw,
} from "lucide-react";
import { commands, TranscriptionProfile } from "@/bindings";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { SettingsGroup } from "../ui/SettingsGroup";
import { SettingContainer } from "../ui/SettingContainer";
import { Button } from "../ui/Button";
import { Input } from "../ui/Input";
import { Dropdown } from "../ui/Dropdown";
import { Badge } from "../ui/Badge";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { HandyShortcut } from "./HandyShortcut";
import { useSettings } from "../../hooks/useSettings";
import { LANGUAGES } from "../../lib/constants/languages";
import { getModelPromptInfo } from "./TranscriptionSystemPrompt";

interface ExtendedTranscriptionProfile extends TranscriptionProfile {
  include_in_cycle: boolean;
  push_to_talk: boolean;
}

interface ProfileCardProps {
  profile: ExtendedTranscriptionProfile;
  isExpanded: boolean;
  onToggleExpand: () => void;
  onUpdate: (profile: ExtendedTranscriptionProfile) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
  canDelete: boolean;
  promptLimit: number;
  isActive: boolean;
  onSetActive: (id: string) => Promise<void>;
}

const ProfileCard: React.FC<ProfileCardProps> = ({
  profile,
  isExpanded,
  onToggleExpand,
  onUpdate,
  onDelete,
  canDelete,
  promptLimit,
  isActive,
  onSetActive,
}) => {
  const { t } = useTranslation();
  const [isUpdating, setIsUpdating] = useState(false);
  const [localName, setLocalName] = useState(profile.name);
  const [localLanguage, setLocalLanguage] = useState(profile.language);
  const [localTranslate, setLocalTranslate] = useState(
    profile.translate_to_english,
  );
  const [localSystemPrompt, setLocalSystemPrompt] = useState(
    profile.system_prompt || "",
  );
  const [localIncludeInCycle, setLocalIncludeInCycle] = useState(
    profile.include_in_cycle,
  );
  const [localPushToTalk, setLocalPushToTalk] = useState(
    profile.push_to_talk ?? true,
  );

  const bindingId = `transcribe_${profile.id}`;

  const languageLabel = useMemo(() => {
    const lang = LANGUAGES.find((l) => l.value === localLanguage);
    return lang?.label || t("settings.general.language.auto");
  }, [localLanguage, t]);

  const promptLength = localSystemPrompt.length;
  const isOverLimit = promptLimit > 0 && promptLength > promptLimit;

  const handleSave = async () => {
    if (!localName.trim()) return;
    if (isOverLimit) return;
    setIsUpdating(true);
    try {
      await onUpdate({
        ...profile,
        name: localName.trim(),
        language: localLanguage,
        translate_to_english: localTranslate,
        system_prompt: localSystemPrompt,
        include_in_cycle: localIncludeInCycle,
        push_to_talk: localPushToTalk,
      });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleDelete = async () => {
    setIsUpdating(true);
    try {
      await onDelete(profile.id);
    } finally {
      setIsUpdating(false);
    }
  };

  const isDirty =
    localName.trim() !== profile.name ||
    localLanguage !== profile.language ||
    localTranslate !== profile.translate_to_english ||
    localSystemPrompt !== profile.system_prompt ||
    localIncludeInCycle !== profile.include_in_cycle ||
    localPushToTalk !== (profile.push_to_talk ?? true);

  return (
    <div
      className={`border rounded-lg transition-colors ${isActive ? "border-purple-500/50 bg-purple-500/5" : "border-mid-gray/30 bg-background/50"}`}
    >
      {/* Header - always visible */}
      <div
        className="flex items-center justify-between px-4 py-3 cursor-pointer hover:bg-mid-gray/5 transition-colors"
        onClick={onToggleExpand}
      >
        <div className="flex items-center gap-3">
          <Globe
            className={`w-4 h-4 ${isActive ? "text-purple-400" : "text-logo-primary"}`}
          />
          <div className="flex flex-col">
            <div className="flex items-center gap-2">
              <span className="font-medium text-sm">{profile.name}</span>
              {isActive && (
                <Badge
                  variant="secondary"
                  className="bg-purple-500/20 text-purple-400 border-purple-500/30 text-[10px] px-1.5 py-0"
                >
                  {t("settings.transcriptionProfiles.active")}
                </Badge>
              )}
            </div>
            <span className="text-xs text-mid-gray">
              {languageLabel}
              {profile.translate_to_english && (
                <span className="text-purple-400 ml-1">→ EN</span>
              )}
            </span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {isExpanded ? (
            <ChevronUp className="w-4 h-4 text-mid-gray" />
          ) : (
            <ChevronDown className="w-4 h-4 text-mid-gray" />
          )}
        </div>
      </div>

      {/* Expanded content */}
      {isExpanded && (
        <div className="px-4 pb-4 pt-2 border-t border-mid-gray/20 space-y-4">
          {/* Active Profile & Cycle Controls */}
          <div className="flex items-center gap-4 bg-mid-gray/5 p-3 rounded-lg border border-mid-gray/10">
            <div className="flex-1">
              <label className="text-xs font-semibold text-text/70 block mb-1">
                {t("settings.transcriptionProfiles.active")}
              </label>
              <Button
                onClick={(e) => {
                  e.stopPropagation();
                  onSetActive(profile.id);
                }}
                disabled={isActive || isUpdating}
                variant={isActive ? "secondary" : "primary"}
                size="sm"
                className={`w-full justify-center ${isActive ? "opacity-100 cursor-default" : ""}`}
              >
                {isActive ? (
                  <>
                    <Check className="w-3.5 h-3.5 mr-1.5" />
                    {t("settings.transcriptionProfiles.active")}
                  </>
                ) : (
                  <>
                    <Play className="w-3.5 h-3.5 mr-1.5" />
                    {t("settings.transcriptionProfiles.setActive")}
                  </>
                )}
              </Button>
            </div>

            <div className="flex-1">
              <label className="text-xs font-semibold text-text/70 block mb-1">
                {t("settings.transcriptionProfiles.includeInCycle")}
              </label>
              <div className="flex items-center gap-2 h-8">
                <input
                  type="checkbox"
                  checked={localIncludeInCycle}
                  onChange={(e) => setLocalIncludeInCycle(e.target.checked)}
                  disabled={isUpdating}
                  className="w-4 h-4 rounded border-mid-gray bg-background text-purple-500 focus:ring-purple-500/50"
                />
                <span className="text-xs text-mid-gray">
                  {t(
                    "settings.transcriptionProfiles.includeInCycleDescription",
                  )}
                </span>
              </div>
            </div>

            <div className="flex-1">
              <label className="text-xs font-semibold text-text/70 block mb-1">
                {t("settings.general.pushToTalk.label")}
              </label>
              <div className="flex items-center gap-2 h-8">
                <ToggleSwitch
                  checked={localPushToTalk}
                  onChange={setLocalPushToTalk}
                  disabled={isUpdating}
                />
                <span className="text-xs text-mid-gray">
                  {t("settings.general.pushToTalk.description")}
                </span>
              </div>
            </div>
          </div>

          {/* Shortcut */}
          <div className="space-y-2">
            <label className="text-xs font-semibold text-text/70">
              {t("settings.transcriptionProfiles.shortcut")}
            </label>
            <HandyShortcut shortcutId={bindingId} grouped={true} />
          </div>

          {/* Profile Name */}
          <div className="space-y-2">
            <label className="text-xs font-semibold text-text/70">
              {t("settings.transcriptionProfiles.profileName")}
            </label>
            <Input
              type="text"
              value={localName}
              onChange={(e) => setLocalName(e.target.value)}
              placeholder={t(
                "settings.transcriptionProfiles.profileNamePlaceholder",
              )}
              variant="compact"
              disabled={isUpdating}
            />
          </div>

          {/* Language Selection */}
          <div className="space-y-2 relative z-20">
            <label className="text-xs font-semibold text-text/70">
              {t("settings.transcriptionProfiles.language")}
            </label>
            <Dropdown
              selectedValue={localLanguage}
              options={LANGUAGES.map((l) => ({
                value: l.value,
                label: l.label,
              }))}
              onSelect={(value) => value && setLocalLanguage(value)}
              placeholder={t("settings.general.language.auto")}
              disabled={isUpdating}
            />
          </div>

          {/* System Prompt */}
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.systemPrompt")}
              </label>
              <span
                className={`text-xs ${isOverLimit ? "text-red-400" : "text-mid-gray"}`}
              >
                {promptLength}
                {promptLimit > 0 && ` / ${promptLimit}`}
              </span>
            </div>
            <textarea
              value={localSystemPrompt}
              onChange={(e) => setLocalSystemPrompt(e.target.value)}
              placeholder={t(
                "settings.transcriptionProfiles.systemPromptPlaceholder",
              )}
              disabled={isUpdating}
              rows={3}
              className={`w-full px-3 py-2 text-sm bg-[#1e1e1e]/80 border rounded-md resize-none transition-colors ${
                isOverLimit
                  ? "border-red-400 focus:border-red-400"
                  : "border-[#3c3c3c] focus:border-[#4a4a4a]"
              } ${isUpdating ? "opacity-40 cursor-not-allowed" : ""} text-[#e8e8e8] placeholder-[#6b6b6b]`}
            />
            <p className="text-xs text-mid-gray">
              {t("settings.transcriptionProfiles.systemPromptDescription")}
            </p>
            {isOverLimit && (
              <p className="text-xs text-red-400">
                {t("settings.transcriptionProfiles.systemPromptTooLong", {
                  limit: promptLimit,
                })}
              </p>
            )}
          </div>

          {/* Translate to English Toggle */}
          <div className="flex items-center justify-between">
            <div>
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.translateToEnglish")}
              </label>
              <p className="text-xs text-mid-gray mt-0.5">
                {t(
                  "settings.transcriptionProfiles.translateToEnglishDescription",
                )}
              </p>
            </div>
            <button
              type="button"
              onClick={() => setLocalTranslate(!localTranslate)}
              disabled={isUpdating}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                localTranslate ? "bg-purple-500" : "bg-mid-gray/30"
              } ${isUpdating ? "opacity-50 cursor-not-allowed" : "cursor-pointer"}`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  localTranslate ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
          </div>

          {/* Action Buttons */}
          <div className="flex gap-2 pt-2">
            <Button
              onClick={handleSave}
              variant="primary"
              size="sm"
              disabled={
                !isDirty || !localName.trim() || isUpdating || isOverLimit
              }
            >
              {t("settings.transcriptionProfiles.saveChanges")}
            </Button>
            {canDelete && (
              <Button
                onClick={handleDelete}
                variant="secondary"
                size="sm"
                disabled={isUpdating}
                className="text-red-400 hover:text-red-300 hover:border-red-400/50"
              >
                <Trash2 className="w-4 h-4" />
              </Button>
            )}
          </div>
        </div>
      )}
    </div>
  );
};

export const TranscriptionProfiles: React.FC = () => {
  const { t } = useTranslation();
  const { settings, refreshSettings, updateSetting } = useSettings();
  const [expandedId, setExpandedId] = useState<string | null>("default");
  const [isCreating, setIsCreating] = useState(false);
  const [newName, setNewName] = useState("");
  const [newLanguage, setNewLanguage] = useState("auto");
  const [newTranslate, setNewTranslate] = useState(false);
  const [newSystemPrompt, setNewSystemPrompt] = useState("");
  const [newPushToTalk, setNewPushToTalk] = useState(true);

  const profiles = (settings?.transcription_profiles ||
    []) as ExtendedTranscriptionProfile[];
  const activeProfileId = (settings as any)?.active_profile_id || "default";
  const overlayEnabled =
    (settings as any)?.profile_switch_overlay_enabled ?? true;

  // Listen for active profile changes from shortcuts
  useEffect(() => {
    const unlistenPromise = listen("active-profile-changed", () => {
      refreshSettings();
    });

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [refreshSettings]);

  // Compute the active model ID based on provider - used for prompt limits and prompt storage
  const activeModelId = useMemo(() => {
    const isRemote =
      settings?.transcription_provider === "remote_openai_compatible";
    return isRemote
      ? settings?.remote_stt?.model_id || ""
      : settings?.selected_model || "";
  }, [
    settings?.transcription_provider,
    settings?.remote_stt?.model_id,
    settings?.selected_model,
  ]);

  // Get prompt limit based on active transcription settings
  const promptLimit = useMemo(() => {
    const info = getModelPromptInfo(activeModelId);
    return info.supportsPrompt ? info.charLimit : 0;
  }, [activeModelId]);

  const newPromptLength = newSystemPrompt.length;
  const isNewPromptOverLimit = promptLimit > 0 && newPromptLength > promptLimit;

  const handleCreate = async () => {
    if (!newName.trim()) return;
    if (isNewPromptOverLimit) return;
    setIsCreating(true);
    try {
      const result = await commands.addTranscriptionProfile(
        newName.trim(),
        newLanguage,
        newTranslate,
        newSystemPrompt,
        newPushToTalk,
      );
      if (result.status === "ok") {
        await refreshSettings();
        setNewName("");
        setNewLanguage("auto");
        setNewTranslate(false);
        setNewSystemPrompt("");
        setNewPushToTalk(true);
        setExpandedId(result.data.id);
      }
    } catch (error) {
      console.error("Failed to create profile:", error);
    } finally {
      setIsCreating(false);
    }
  };

  const handleUpdate = async (profile: ExtendedTranscriptionProfile) => {
    try {
      await invoke("update_transcription_profile", {
        id: profile.id,
        name: profile.name,
        language: profile.language,
        translateToEnglish: profile.translate_to_english,
        systemPrompt: profile.system_prompt || "",
        includeInCycle: profile.include_in_cycle,
        pushToTalk: profile.push_to_talk,
      });
      await refreshSettings();
    } catch (error) {
      console.error("Failed to update profile:", error);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await commands.deleteTranscriptionProfile(id);
      await refreshSettings();
      if (expandedId === id) {
        setExpandedId(null);
      }
    } catch (error) {
      console.error("Failed to delete profile:", error);
    }
  };

  const handleSetActive = async (id: string) => {
    try {
      await invoke("set_active_profile", { id });
      await refreshSettings();
    } catch (e) {
      console.error("Failed to set active profile", e);
    }
  };

  const handleOverlayChange = async (enabled: boolean) => {
    if (updateSetting) {
      await updateSetting("profile_switch_overlay_enabled" as any, enabled);
    }
  };

  return (
    <SettingsGroup title={t("settings.transcriptionProfiles.title")}>
      {/* Help text */}
      <SettingContainer
        title=""
        description=""
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="p-3 bg-purple-500/10 border border-purple-500/30 rounded-lg">
          <p className="text-sm text-text/80">
            {t("settings.transcriptionProfiles.help")}
          </p>
        </div>
      </SettingContainer>

      {/* General Settings: Cycle Shortcut & Overlay */}
      <SettingContainer
        title="General Settings"
        description=""
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="space-y-4">
          {/* Cycle Shortcut */}
          <div className="flex items-center justify-between">
            <div className="flex flex-col">
              <span className="text-sm font-medium">
                {t("settings.transcriptionProfiles.includeInCycleDescription")}
              </span>
              <span className="text-xs text-mid-gray">
                Global shortcut to cycle through active profiles
              </span>
            </div>
            <HandyShortcut shortcutId="cycle_profile" />
          </div>

          {/* Overlay Toggle */}
          <div className="flex items-center justify-between">
            <div className="flex flex-col">
              <span className="text-sm font-medium">
                {t("settings.transcriptionProfiles.showOverlayOnSwitch")}
              </span>
              <span className="text-xs text-mid-gray">
                {t(
                  "settings.transcriptionProfiles.showOverlayOnSwitchDescription",
                )}
              </span>
            </div>
            <ToggleSwitch
              checked={overlayEnabled}
              onChange={handleOverlayChange}
            />
          </div>
        </div>
      </SettingContainer>

      <SettingContainer
        title={t("settings.transcriptionProfiles.existingProfiles")}
        description=""
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="space-y-2">
          {/* Default Profile Card - Expandable with global settings */}
          <div
            className={`border rounded-lg transition-colors ${activeProfileId === "default" ? "border-purple-500/50 bg-purple-500/5" : "border-mid-gray/30 bg-background/50"}`}
          >
            {/* Header - clickable to expand */}
            <div
              className="flex items-center justify-between px-4 py-3 cursor-pointer hover:bg-mid-gray/5 transition-colors"
              onClick={() =>
                setExpandedId(expandedId === "default" ? null : "default")
              }
            >
              <div className="flex items-center gap-3">
                <RefreshCw
                  className={`w-4 h-4 ${activeProfileId === "default" ? "text-purple-400" : "text-mid-gray"}`}
                />
                <div className="flex flex-col">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-sm">
                      {t("settings.transcriptionProfiles.defaultProfile")}
                    </span>
                    {activeProfileId === "default" && (
                      <Badge
                        variant="secondary"
                        className="bg-purple-500/20 text-purple-400 border-purple-500/30 text-[10px] px-1.5 py-0"
                      >
                        {t("settings.transcriptionProfiles.active")}
                      </Badge>
                    )}
                  </div>
                  <span className="text-xs text-mid-gray">
                    {t(
                      "settings.transcriptionProfiles.defaultProfileDescription",
                    )}
                  </span>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <Button
                  onClick={(e) => {
                    e.stopPropagation();
                    handleSetActive("default");
                  }}
                  disabled={activeProfileId === "default"}
                  variant={
                    activeProfileId === "default" ? "secondary" : "primary"
                  }
                  size="sm"
                  className={
                    activeProfileId === "default"
                      ? "opacity-100 cursor-default"
                      : ""
                  }
                >
                  {activeProfileId === "default" ? (
                    <Check className="w-4 h-4" />
                  ) : (
                    <span className="text-xs">
                      {t("settings.transcriptionProfiles.setActive")}
                    </span>
                  )}
                </Button>
                {expandedId === "default" ? (
                  <ChevronUp className="w-4 h-4 text-mid-gray" />
                ) : (
                  <ChevronDown className="w-4 h-4 text-mid-gray" />
                )}
              </div>
            </div>

            {/* Expanded content - Global settings */}
            {expandedId === "default" && (
              <div className="px-4 pb-4 pt-2 border-t border-mid-gray/20 space-y-4">
                {/* Shortcut */}
                <div className="space-y-2">
                  <label className="text-xs font-semibold text-text/70">
                    {t("settings.transcriptionProfiles.shortcut")}
                  </label>
                  <HandyShortcut shortcutId="transcribe" grouped={true} />
                </div>

                {/* Language & Push-to-Talk in a row */}
                <div className="grid grid-cols-2 gap-4">
                  <div className="space-y-2">
                    <label className="text-xs font-semibold text-text/70">
                      {t("settings.general.language.title")}
                    </label>
                    <Dropdown
                      selectedValue={settings?.selected_language || "auto"}
                      onSelect={(value: string) =>
                        updateSetting &&
                        updateSetting("selected_language" as any, value)
                      }
                      options={LANGUAGES.map((lang) => ({
                        value: lang.value,
                        label: lang.label,
                      }))}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-xs font-semibold text-text/70">
                      {t("settings.general.pushToTalk.label")}
                    </label>
                    <div className="flex items-center gap-2 h-9">
                      <ToggleSwitch
                        checked={settings?.push_to_talk ?? true}
                        onChange={(checked) =>
                          updateSetting &&
                          updateSetting("push_to_talk" as any, checked)
                        }
                      />
                      <span className="text-xs text-mid-gray">
                        {t("settings.general.pushToTalk.description")}
                      </span>
                    </div>
                  </div>
                </div>

                {/* System Prompt */}
                <div className="space-y-2">
                  <div className="flex items-center justify-between">
                    <label className="text-xs font-semibold text-text/70">
                      {t("settings.general.transcriptionSystemPrompt.title")}
                    </label>
                    {promptLimit > 0 && (
                      <span
                        className={`text-xs ${(settings?.transcription_prompts?.[activeModelId] || "").length > promptLimit ? "text-red-400" : "text-mid-gray"}`}
                      >
                        {
                          (
                            settings?.transcription_prompts?.[
                              activeModelId
                            ] || ""
                          ).length
                        }
                        /{promptLimit}
                      </span>
                    )}
                  </div>
                  <textarea
                    value={
                      settings?.transcription_prompts?.[
                        activeModelId
                      ] || ""
                    }
                    onChange={async (e) => {
                      if (activeModelId) {
                        await commands.changeTranscriptionPromptSetting(
                          activeModelId,
                          e.target.value,
                        );
                        refreshSettings();
                      }
                    }}
                    placeholder={t(
                      "settings.general.transcriptionSystemPrompt.placeholder",
                    )}
                    className="w-full h-20 px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm text-text placeholder-mid-gray/50 resize-none focus:outline-none focus:border-purple-500/50"
                  />
                </div>
              </div>
            )}
          </div>

          {/* Custom Profiles */}
          {profiles.map((profile) => (
            <ProfileCard
              key={profile.id}
              profile={profile}
              isExpanded={expandedId === profile.id}
              onToggleExpand={() =>
                setExpandedId(expandedId === profile.id ? null : profile.id)
              }
              onUpdate={handleUpdate}
              onDelete={handleDelete}
              canDelete={true}
              promptLimit={promptLimit}
              isActive={activeProfileId === profile.id}
              onSetActive={handleSetActive}
            />
          ))}
        </div>
      </SettingContainer>

      {/* Create new profile */}
      <SettingContainer
        title={t("settings.transcriptionProfiles.createNew")}
        description={t("settings.transcriptionProfiles.createNewDescription")}
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="space-y-3 p-3 border border-dashed border-mid-gray/30 rounded-lg overflow-visible">
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.profileName")}
              </label>
              <Input
                type="text"
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                placeholder={t(
                  "settings.transcriptionProfiles.profileNamePlaceholder",
                )}
                variant="compact"
                disabled={isCreating}
              />
            </div>
            <div className="space-y-1 relative z-10">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.language")}
              </label>
              <Dropdown
                selectedValue={newLanguage}
                options={LANGUAGES.map((l) => ({
                  value: l.value,
                  label: l.label,
                }))}
                onSelect={(value) => value && setNewLanguage(value)}
                placeholder={t("settings.general.language.auto")}
                disabled={isCreating}
              />
            </div>
          </div>

          {/* System Prompt for new profile */}
          <div className="space-y-1">
            <div className="flex items-center justify-between">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.systemPrompt")}
              </label>
              <span
                className={`text-xs ${isNewPromptOverLimit ? "text-red-400" : "text-mid-gray"}`}
              >
                {newPromptLength}
                {promptLimit > 0 && ` / ${promptLimit}`}
              </span>
            </div>
            <textarea
              value={newSystemPrompt}
              onChange={(e) => setNewSystemPrompt(e.target.value)}
              placeholder={t(
                "settings.transcriptionProfiles.systemPromptPlaceholder",
              )}
              disabled={isCreating}
              rows={2}
              className={`w-full px-3 py-2 text-sm bg-[#1e1e1e]/80 border rounded-md resize-none transition-colors ${
                isNewPromptOverLimit
                  ? "border-red-400 focus:border-red-400"
                  : "border-[#3c3c3c] focus:border-[#4a4a4a]"
              } ${isCreating ? "opacity-40 cursor-not-allowed" : ""} text-[#e8e8e8] placeholder-[#6b6b6b]`}
            />
            {isNewPromptOverLimit && (
              <p className="text-xs text-red-400">
                {t("settings.transcriptionProfiles.systemPromptTooLong", {
                  limit: promptLimit,
                })}
              </p>
            )}
          </div>

          <div className="flex items-center justify-between">
            <div>
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.translateToEnglish")}
              </label>
            </div>
            <button
              type="button"
              onClick={() => setNewTranslate(!newTranslate)}
              disabled={isCreating}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                newTranslate ? "bg-purple-500" : "bg-mid-gray/30"
              } ${isCreating ? "opacity-50 cursor-not-allowed" : "cursor-pointer"}`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  newTranslate ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
          </div>

          <div className="flex items-center justify-between">
            <div>
              <label className="text-xs font-semibold text-text/70">
                {t("settings.general.pushToTalk.label")}
              </label>
            </div>
            <button
              type="button"
              onClick={() => setNewPushToTalk(!newPushToTalk)}
              disabled={isCreating}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                newPushToTalk ? "bg-purple-500" : "bg-mid-gray/30"
              } ${isCreating ? "opacity-50 cursor-not-allowed" : "cursor-pointer"}`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  newPushToTalk ? "translate-x-6" : "translate-x-1"
                }`}
              />
            </button>
          </div>

          {/* Create Button */}
          <div className="flex justify-end pt-1">
            <Button
              onClick={handleCreate}
              variant="primary"
              size="sm"
              disabled={!newName.trim() || isCreating || isNewPromptOverLimit}
              className="inline-flex items-center"
            >
              <Plus className="w-3.5 h-3.5 mr-1.5" />
              {t("settings.transcriptionProfiles.addProfile")}
            </Button>
          </div>
        </div>
      </SettingContainer>
    </SettingsGroup>
  );
};
