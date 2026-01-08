import React, { useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Trash2, ChevronDown, ChevronUp, Globe } from "lucide-react";
import { commands, TranscriptionProfile } from "@/bindings";

import { SettingsGroup } from "../ui/SettingsGroup";
import { SettingContainer } from "../ui/SettingContainer";
import { Button } from "../ui/Button";
import { Input } from "../ui/Input";
import { Dropdown } from "../ui/Dropdown";
import { HandyShortcut } from "./HandyShortcut";
import { useSettings } from "../../hooks/useSettings";
import { LANGUAGES } from "../../lib/constants/languages";
import { getModelPromptInfo } from "./TranscriptionSystemPrompt";

interface ProfileCardProps {
  profile: TranscriptionProfile;
  isExpanded: boolean;
  onToggleExpand: () => void;
  onUpdate: (profile: TranscriptionProfile) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
  canDelete: boolean;
  promptLimit: number;
}

const ProfileCard: React.FC<ProfileCardProps> = ({
  profile,
  isExpanded,
  onToggleExpand,
  onUpdate,
  onDelete,
  canDelete,
  promptLimit,
}) => {
  const { t } = useTranslation();
  const [isUpdating, setIsUpdating] = useState(false);
  const [localName, setLocalName] = useState(profile.name);
  const [localLanguage, setLocalLanguage] = useState(profile.language);
  const [localTranslate, setLocalTranslate] = useState(profile.translate_to_english);
  const [localSystemPrompt, setLocalSystemPrompt] = useState(profile.system_prompt || "");

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
    localSystemPrompt !== profile.system_prompt;

  return (
    <div className="border border-mid-gray/30 rounded-lg bg-background/50">
      {/* Header - always visible */}
      <div
        className="flex items-center justify-between px-4 py-3 cursor-pointer hover:bg-mid-gray/5 transition-colors"
        onClick={onToggleExpand}
      >
        <div className="flex items-center gap-3">
          <Globe className="w-4 h-4 text-logo-primary" />
          <div>
            <span className="font-medium text-sm">{profile.name}</span>
            <span className="text-xs text-mid-gray ml-2">
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
              placeholder={t("settings.transcriptionProfiles.profileNamePlaceholder")}
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
              options={LANGUAGES.map((l) => ({ value: l.value, label: l.label }))}
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
              <span className={`text-xs ${isOverLimit ? "text-red-400" : "text-mid-gray"}`}>
                {promptLength}
                {promptLimit > 0 && ` / ${promptLimit}`}
              </span>
            </div>
            <textarea
              value={localSystemPrompt}
              onChange={(e) => setLocalSystemPrompt(e.target.value)}
              placeholder={t("settings.transcriptionProfiles.systemPromptPlaceholder")}
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
                {t("settings.transcriptionProfiles.systemPromptTooLong", { limit: promptLimit })}
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
                {t("settings.transcriptionProfiles.translateToEnglishDescription")}
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
              disabled={!isDirty || !localName.trim() || isUpdating || isOverLimit}
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
  const { settings, refreshSettings } = useSettings();
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [newName, setNewName] = useState("");
  const [newLanguage, setNewLanguage] = useState("auto");
  const [newTranslate, setNewTranslate] = useState(false);
  const [newSystemPrompt, setNewSystemPrompt] = useState("");

  const profiles = settings?.transcription_profiles || [];

  // Get prompt limit based on active transcription settings
  // Profiles use the same model as the main transcription
  const promptLimit = useMemo(() => {
    const isRemote = settings?.transcription_provider === "remote_openai_compatible";
    const modelId = isRemote
      ? settings?.remote_stt?.model_id || ""
      : settings?.selected_model || "";
    const info = getModelPromptInfo(modelId);
    return info.supportsPrompt ? info.charLimit : 0;
  }, [settings?.transcription_provider, settings?.remote_stt?.model_id, settings?.selected_model]);

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
        newSystemPrompt
      );
      if (result.status === "ok") {
        await refreshSettings();
        setNewName("");
        setNewLanguage("auto");
        setNewTranslate(false);
        setNewSystemPrompt("");
        setExpandedId(result.data.id);
      }
    } catch (error) {
      console.error("Failed to create profile:", error);
    } finally {
      setIsCreating(false);
    }
  };

  const handleUpdate = async (profile: TranscriptionProfile) => {
    try {
      await commands.updateTranscriptionProfile(
        profile.id,
        profile.name,
        profile.language,
        profile.translate_to_english,
        profile.system_prompt || ""
      );
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

      {/* Existing profiles */}
      {profiles.length > 0 && (
        <SettingContainer
          title={t("settings.transcriptionProfiles.existingProfiles")}
          description=""
          descriptionMode="inline"
          layout="stacked"
          grouped={true}
        >
          <div className="space-y-2">
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
              />
            ))}
          </div>
        </SettingContainer>
      )}

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
                placeholder={t("settings.transcriptionProfiles.profileNamePlaceholder")}
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
                options={LANGUAGES.map((l) => ({ value: l.value, label: l.label }))}
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
              <span className={`text-xs ${isNewPromptOverLimit ? "text-red-400" : "text-mid-gray"}`}>
                {newPromptLength}
                {promptLimit > 0 && ` / ${promptLimit}`}
              </span>
            </div>
            <textarea
              value={newSystemPrompt}
              onChange={(e) => setNewSystemPrompt(e.target.value)}
              placeholder={t("settings.transcriptionProfiles.systemPromptPlaceholder")}
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
                {t("settings.transcriptionProfiles.systemPromptTooLong", { limit: promptLimit })}
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
