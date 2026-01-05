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

interface ProfileCardProps {
  profile: TranscriptionProfile;
  isExpanded: boolean;
  onToggleExpand: () => void;
  onUpdate: (profile: TranscriptionProfile) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
  canDelete: boolean;
}

const ProfileCard: React.FC<ProfileCardProps> = ({
  profile,
  isExpanded,
  onToggleExpand,
  onUpdate,
  onDelete,
  canDelete,
}) => {
  const { t } = useTranslation();
  const [isUpdating, setIsUpdating] = useState(false);
  const [localName, setLocalName] = useState(profile.name);
  const [localLanguage, setLocalLanguage] = useState(profile.language);
  const [localTranslate, setLocalTranslate] = useState(profile.translate_to_english);

  const bindingId = `transcribe_${profile.id}`;

  const languageLabel = useMemo(() => {
    const lang = LANGUAGES.find((l) => l.value === localLanguage);
    return lang?.label || t("settings.general.language.auto");
  }, [localLanguage, t]);

  const handleSave = async () => {
    if (!localName.trim()) return;
    setIsUpdating(true);
    try {
      await onUpdate({
        ...profile,
        name: localName.trim(),
        language: localLanguage,
        translate_to_english: localTranslate,
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
    localTranslate !== profile.translate_to_english;

  return (
    <div className="border border-mid-gray/30 rounded-lg bg-background/50 overflow-hidden">
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
          <div className="space-y-2">
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
              disabled={!isDirty || !localName.trim() || isUpdating}
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

  const profiles = settings?.transcription_profiles || [];

  const handleCreate = async () => {
    if (!newName.trim()) return;
    setIsCreating(true);
    try {
      const result = await commands.addTranscriptionProfile(
        newName.trim(),
        newLanguage,
        newTranslate
      );
      if (result.status === "ok") {
        await refreshSettings();
        setNewName("");
        setNewLanguage("auto");
        setNewTranslate(false);
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
        profile.translate_to_english
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
        <div className="space-y-3 p-3 border border-dashed border-mid-gray/30 rounded-lg">
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
            <div className="space-y-1">
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

          <Button
            onClick={handleCreate}
            variant="primary"
            size="md"
            disabled={!newName.trim() || isCreating}
            className="w-full"
          >
            <Plus className="w-4 h-4 mr-2" />
            {t("settings.transcriptionProfiles.addProfile")}
          </Button>
        </div>
      </SettingContainer>
    </SettingsGroup>
  );
};
