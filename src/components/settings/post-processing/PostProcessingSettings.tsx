import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCcw } from "lucide-react";
import { commands } from "@/bindings";

import { SettingsGroup } from "../../ui/SettingsGroup";
import { TellMeMore } from "../../ui/TellMeMore";
import { SettingContainer } from "../../ui/SettingContainer";
import { Button } from "../../ui/Button";
import { ResetButton } from "../../ui/ResetButton";
import { Input } from "../../ui/Input";
import { Dropdown } from "../../ui/Dropdown";
import { Textarea } from "../../ui/Textarea";
import { PostProcessingToggle } from "../PostProcessingToggle";
import { ProviderSelect } from "../PostProcessingSettingsApi/ProviderSelect";
import { BaseUrlField } from "../PostProcessingSettingsApi/BaseUrlField";
import { ApiKeyField } from "../PostProcessingSettingsApi/ApiKeyField";
import { ModelSelect } from "../PostProcessingSettingsApi/ModelSelect";
import { usePostProcessProviderState } from "../PostProcessingSettingsApi/usePostProcessProviderState";
import { useAiReplaceProviderState } from "./useAiReplaceProviderState";
import { useSettings } from "../../../hooks/useSettings";
import { ExtendedThinkingSection } from "../ExtendedThinkingSection";

const DisabledNotice: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => (
  <div className="p-4 bg-mid-gray/5 rounded-lg border border-mid-gray/20">
    <p className="text-sm text-mid-gray">{children}</p>
  </div>
);

const LlmConfigSection: React.FC<{
  title: string;
  description: string;
  state: any; // Can be from usePostProcessProviderState or useAiReplaceProviderState
  showBaseUrl?: boolean;
  /** Setting prefix for Extended Thinking: "post_process" or "ai_replace" */
  reasoningSettingPrefix?: "post_process" | "ai_replace";
}> = ({ title, description, state, showBaseUrl = true, reasoningSettingPrefix }) => {
  const { t } = useTranslation();

  return (
    <div className="space-y-4 pt-4">
      <div className="px-6">
        <h3 className="text-sm font-semibold text-text">{title}</h3>
        <p className="text-xs text-text/60 mt-1">{description}</p>
      </div>

      <SettingContainer
        title={t("settings.postProcessing.api.provider.title")}
        description={t("settings.postProcessing.api.provider.description")}
        descriptionMode="tooltip"
        layout="horizontal"
        grouped={true}
      >
        <div className="flex items-center gap-2">
          <ProviderSelect
            options={state.providerOptions}
            value={state.selectedProviderId}
            onChange={state.handleProviderSelect}
          />
        </div>
      </SettingContainer>

      {state.appleIntelligenceUnavailable ? (
        <div className="p-3 bg-red-500/10 border border-red-500/50">
          <p className="text-sm text-red-500">
            {t("settings.postProcessing.api.appleIntelligence.unavailable")}
          </p>
        </div>
      ) : null}

      {/* If state has useSameAsPostProcess and it's true, show notice and hide rest */}
      {state.useSameAsPostProcess ? (
        <SettingContainer
          title=""
          description=""
          descriptionMode="tooltip"
          layout="stacked"
          grouped={true}
        >
          <div className="p-3 bg-purple-500/5 border border-purple-500/20 rounded-lg">
            <p className="text-xs text-purple-400/80">
              {t("settings.postProcessing.api.aiReplace.sameAsPostProcessing")}
            </p>
          </div>
        </SettingContainer>
      ) : (
        <>
          {state.isAppleProvider ? (
            <SettingContainer
              title={t("settings.postProcessing.api.appleIntelligence.title")}
              description={t(
                "settings.postProcessing.api.appleIntelligence.description",
              )}
              descriptionMode="tooltip"
              layout="stacked"
              grouped={true}
            >
              <DisabledNotice>
                {t(
                  "settings.postProcessing.api.appleIntelligence.requirements",
                )}
              </DisabledNotice>
            </SettingContainer>
          ) : (
            <>
              {showBaseUrl && state.selectedProvider?.id === "custom" && (
                <SettingContainer
                  title={t("settings.postProcessing.api.baseUrl.title")}
                  description={t(
                    "settings.postProcessing.api.baseUrl.description",
                  )}
                  descriptionMode="tooltip"
                  layout="horizontal"
                  grouped={true}
                >
                  <div className="flex items-center gap-2">
                    <BaseUrlField
                      value={state.baseUrl}
                      onBlur={state.handleBaseUrlChange || (() => {})}
                      placeholder={t(
                        "settings.postProcessing.api.baseUrl.placeholder",
                      )}
                      disabled={state.isBaseUrlUpdating}
                      className="min-w-[380px]"
                    />
                  </div>
                </SettingContainer>
              )}

              <SettingContainer
                title={t("settings.postProcessing.api.apiKey.title")}
                description={t(
                  "settings.postProcessing.api.apiKey.description",
                )}
                descriptionMode="tooltip"
                layout="horizontal"
                grouped={true}
              >
                <div className="flex items-center gap-2">
                  <ApiKeyField
                    value={state.apiKey}
                    onBlur={state.handleApiKeyChange}
                    placeholder={t(
                      "settings.postProcessing.api.apiKey.placeholder",
                    )}
                    disabled={state.isApiKeyUpdating}
                    className="min-w-[320px]"
                  />
                </div>
              </SettingContainer>
            </>
          )}

          <SettingContainer
            title={t("settings.postProcessing.api.model.title")}
            description={
              state.isAppleProvider
                ? t("settings.postProcessing.api.model.descriptionApple")
                : state.isCustomProvider
                  ? t("settings.postProcessing.api.model.descriptionCustom")
                  : t("settings.postProcessing.api.model.descriptionDefault")
            }
            descriptionMode="tooltip"
            layout="stacked"
            grouped={true}
          >
            <div className="flex items-center gap-2">
              <ModelSelect
                value={state.model}
                options={state.modelOptions}
                disabled={state.isModelUpdating}
                isLoading={state.isFetchingModels}
                placeholder={
                  state.isAppleProvider
                    ? t("settings.postProcessing.api.model.placeholderApple")
                    : state.modelOptions.length > 0
                      ? t(
                          "settings.postProcessing.api.model.placeholderWithOptions",
                        )
                      : t(
                          "settings.postProcessing.api.model.placeholderNoOptions",
                        )
                }
                onSelect={state.handleModelSelect}
                onCreate={state.handleModelCreate}
                onBlur={() => {}}
                className="flex-1 min-w-[380px]"
              />
              <ResetButton
                onClick={state.handleRefreshModels}
                disabled={state.isFetchingModels || state.isAppleProvider}
                ariaLabel={t("settings.postProcessing.api.model.refreshModels")}
                title="Fetch Models from Server, then you can type part of model name for searching it in drop-down."
                className="flex h-10 w-10 items-center justify-center"
              >
                <RefreshCcw
                  className={`h-4 w-4 ${state.isFetchingModels ? "animate-spin" : ""}`}
                />
              </ResetButton>
            </div>
          </SettingContainer>

          {/* Extended Thinking Section - shown for non-Apple providers */}
          {reasoningSettingPrefix && !state.isAppleProvider && (
            <ExtendedThinkingSection settingPrefix={reasoningSettingPrefix} />
          )}
        </>
      )}
    </div>
  );
};

const PostProcessingSettingsApiComponent: React.FC = () => {
  const { t } = useTranslation();
  const postProcessState = usePostProcessProviderState();
  const aiReplaceState = useAiReplaceProviderState();

  return (
    <div className="divide-y divide-mid-gray/10 space-y-6">
      <LlmConfigSection
        title={t("settings.postProcessing.api.transcription.title")}
        description={t("settings.postProcessing.api.transcription.description")}
        state={postProcessState}
        reasoningSettingPrefix="post_process"
      />

      <LlmConfigSection
        title={t("settings.postProcessing.api.aiReplace.title")}
        description={t("settings.postProcessing.api.aiReplace.description")}
        state={aiReplaceState}
        showBaseUrl={false} // Base URL is shared per provider
        reasoningSettingPrefix="ai_replace"
      />
    </div>
  );
};

const PostProcessingSettingsPromptsComponent: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating, refreshSettings } =
    useSettings();
  const [isCreating, setIsCreating] = useState(false);
  const [draftName, setDraftName] = useState("");
  const [draftText, setDraftText] = useState("");

  const enabled = getSetting("post_process_enabled") || false;
  const prompts = getSetting("post_process_prompts") || [];
  const selectedPromptId = getSetting("post_process_selected_prompt_id") || "";
  const selectedPrompt =
    prompts.find((prompt) => prompt.id === selectedPromptId) || null;

  useEffect(() => {
    if (isCreating) return;

    if (selectedPrompt) {
      setDraftName(selectedPrompt.name);
      setDraftText(selectedPrompt.prompt);
    } else {
      setDraftName("");
      setDraftText("");
    }
  }, [
    isCreating,
    selectedPromptId,
    selectedPrompt?.name,
    selectedPrompt?.prompt,
  ]);

  const handlePromptSelect = (promptId: string | null) => {
    if (!promptId) return;
    updateSetting("post_process_selected_prompt_id", promptId);
    setIsCreating(false);
  };

  const handleCreatePrompt = async () => {
    if (!draftName.trim() || !draftText.trim()) return;

    try {
      const result = await commands.addPostProcessPrompt(
        draftName.trim(),
        draftText.trim(),
      );
      if (result.status === "ok") {
        await refreshSettings();
        updateSetting("post_process_selected_prompt_id", result.data.id);
        setIsCreating(false);
      }
    } catch (error) {
      console.error("Failed to create prompt:", error);
    }
  };

  const handleUpdatePrompt = async () => {
    if (!selectedPromptId || !draftName.trim() || !draftText.trim()) return;

    try {
      await commands.updatePostProcessPrompt(
        selectedPromptId,
        draftName.trim(),
        draftText.trim(),
      );
      await refreshSettings();
    } catch (error) {
      console.error("Failed to update prompt:", error);
    }
  };

  const handleDeletePrompt = async (promptId: string) => {
    if (!promptId) return;

    try {
      await commands.deletePostProcessPrompt(promptId);
      await refreshSettings();
      setIsCreating(false);
    } catch (error) {
      console.error("Failed to delete prompt:", error);
    }
  };

  const handleCancelCreate = () => {
    setIsCreating(false);
    if (selectedPrompt) {
      setDraftName(selectedPrompt.name);
      setDraftText(selectedPrompt.prompt);
    } else {
      setDraftName("");
      setDraftText("");
    }
  };

  const handleStartCreate = () => {
    setIsCreating(true);
    setDraftName("");
    setDraftText("");
  };



  const hasPrompts = prompts.length > 0;
  const isDirty =
    !!selectedPrompt &&
    (draftName.trim() !== selectedPrompt.name ||
      draftText.trim() !== selectedPrompt.prompt.trim());

  return (
    <SettingContainer
      title={t("settings.postProcessing.prompts.selectedPrompt.title")}
      description={t(
        "settings.postProcessing.prompts.selectedPrompt.description",
      )}
      descriptionMode="inline"
      layout="stacked"
      grouped={true}
    >
      <div className="space-y-3">
        <div className="flex gap-2">
          <Dropdown
            selectedValue={selectedPromptId || null}
            options={prompts.map((p) => ({
              value: p.id,
              label: p.name,
            }))}
            onSelect={(value) => handlePromptSelect(value)}
            placeholder={
              prompts.length === 0
                ? t("settings.postProcessing.prompts.noPrompts")
                : t("settings.postProcessing.prompts.selectPrompt")
            }
            disabled={
              isUpdating("post_process_selected_prompt_id") || isCreating
            }
            className="flex-1"
          />
          <Button
            onClick={handleStartCreate}
            variant="primary"
            size="md"
            disabled={isCreating}
          >
            {t("settings.postProcessing.prompts.createNew")}
          </Button>
        </div>

        {!isCreating && hasPrompts && selectedPrompt && (
          <div className="space-y-3">
            <div className="space-y-2 flex flex-col">
              <label className="text-sm font-semibold">
                {t("settings.postProcessing.prompts.promptLabel")}
              </label>
              <Input
                type="text"
                value={draftName}
                onChange={(e) => setDraftName(e.target.value)}
                placeholder={t(
                  "settings.postProcessing.prompts.promptLabelPlaceholder",
                )}
                variant="compact"
              />
            </div>

            <div className="space-y-2 flex flex-col">
              <label className="text-sm font-semibold">
                {t("settings.postProcessing.prompts.promptInstructions")}
              </label>
              <Textarea
                value={draftText}
                onChange={(e) => setDraftText(e.target.value)}
                placeholder={t(
                  "settings.postProcessing.prompts.promptInstructionsPlaceholder",
                )}
              />
              <p
                className="text-xs text-mid-gray/70"
                dangerouslySetInnerHTML={{
                  __html: t("settings.postProcessing.prompts.promptTip"),
                }}
              />
            </div>

            <div className="flex gap-2 pt-2">
              <Button
                onClick={handleUpdatePrompt}
                variant="primary"
                size="md"
                disabled={!draftName.trim() || !draftText.trim() || !isDirty}
              >
                {t("settings.postProcessing.prompts.updatePrompt")}
              </Button>
              <Button
                onClick={() => handleDeletePrompt(selectedPromptId)}
                variant="secondary"
                size="md"
                disabled={!selectedPromptId || prompts.length <= 1}
              >
                {t("settings.postProcessing.prompts.deletePrompt")}
              </Button>
            </div>
          </div>
        )}

        {!isCreating && !selectedPrompt && (
          <div className="p-3 bg-mid-gray/5 rounded border border-mid-gray/20">
            <p className="text-sm text-mid-gray">
              {hasPrompts
                ? t("settings.postProcessing.prompts.selectToEdit")
                : t("settings.postProcessing.prompts.createFirst")}
            </p>
          </div>
        )}

        {isCreating && (
          <div className="space-y-3">
            <div className="space-y-2 block flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.postProcessing.prompts.promptLabel")}
              </label>
              <Input
                type="text"
                value={draftName}
                onChange={(e) => setDraftName(e.target.value)}
                placeholder={t(
                  "settings.postProcessing.prompts.promptLabelPlaceholder",
                )}
                variant="compact"
              />
            </div>

            <div className="space-y-2 flex flex-col">
              <label className="text-sm font-semibold">
                {t("settings.postProcessing.prompts.promptInstructions")}
              </label>
              <Textarea
                value={draftText}
                onChange={(e) => setDraftText(e.target.value)}
                placeholder={t(
                  "settings.postProcessing.prompts.promptInstructionsPlaceholder",
                )}
              />
              <p
                className="text-xs text-mid-gray/70"
                dangerouslySetInnerHTML={{
                  __html: t("settings.postProcessing.prompts.promptTip"),
                }}
              />
            </div>

            <div className="flex gap-2 pt-2">
              <Button
                onClick={handleCreatePrompt}
                variant="primary"
                size="md"
                disabled={!draftName.trim() || !draftText.trim()}
              >
                {t("settings.postProcessing.prompts.createPrompt")}
              </Button>
              <Button
                onClick={handleCancelCreate}
                variant="secondary"
                size="md"
              >
                {t("settings.postProcessing.prompts.cancel")}
              </Button>
            </div>
          </div>
        )}
      </div>
    </SettingContainer>
  );
};

export const PostProcessingSettingsApi = React.memo(
  PostProcessingSettingsApiComponent,
);
PostProcessingSettingsApi.displayName = "PostProcessingSettingsApi";

export const PostProcessingSettingsPrompts = React.memo(
  PostProcessingSettingsPromptsComponent,
);
PostProcessingSettingsPrompts.displayName = "PostProcessingSettingsPrompts";

export const PostProcessingSettings: React.FC = () => {
  const { t } = useTranslation();

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      {/* Help Section */}
      <TellMeMore title={t("settings.postProcessing.tellMeMore.title")}>
        <div className="space-y-3">
          <p>
            <strong>{t("settings.postProcessing.tellMeMore.headline")}</strong>
          </p>
          <p className="opacity-90">
            {t("settings.postProcessing.tellMeMore.intro")}
          </p>
          <ul className="list-disc list-inside space-y-2 ml-1 opacity-90">
            <li>
              <strong>{t("settings.postProcessing.tellMeMore.apiKey.title")}</strong>{" "}
              {t("settings.postProcessing.tellMeMore.apiKey.description")}
              <p className="ml-5 mt-1 text-xs text-text/70 italic">
                {t("settings.postProcessing.tellMeMore.apiKey.securityNote")}
              </p>
            </li>
            <li>
              <strong>{t("settings.postProcessing.tellMeMore.provider.title")}</strong>{" "}
              {t("settings.postProcessing.tellMeMore.provider.description")}
            </li>
            <li>
              <strong>{t("settings.postProcessing.tellMeMore.model.title")}</strong>{" "}
              {t("settings.postProcessing.tellMeMore.model.description")}
            </li>
            <li>
              <strong>{t("settings.postProcessing.tellMeMore.prompts.title")}</strong>{" "}
              {t("settings.postProcessing.tellMeMore.prompts.description")}
            </li>
          </ul>
          <div className="mt-3 p-2 bg-accent/10 border border-accent/20 rounded-md text-xs">
            <p className="mb-1">{t("settings.postProcessing.tellMeMore.tip")}</p>
            <a
              href="https://openrouter.ai"
              target="_blank"
              rel="noopener noreferrer"
              className="text-accent hover:underline font-medium"
            >
              openrouter.ai
            </a>
          </div>
          <div className="mt-3 p-3 bg-red-500/10 border border-red-500/30 rounded-md">
            <p className="text-sm font-semibold text-red-400 mb-1">
              {t("settings.postProcessing.tellMeMore.privacyWarning.title")}
            </p>
            <p className="text-xs text-red-300/90">
              {t("settings.postProcessing.tellMeMore.privacyWarning.description")}
            </p>
          </div>
        </div>
      </TellMeMore>

      <SettingsGroup title={t("settings.postProcessing.prompts.title")}>
        <PostProcessingToggle descriptionMode="inline" grouped={true} />
        <PostProcessingSettingsPrompts />
      </SettingsGroup>

      <SettingsGroup title={t("settings.postProcessing.api.title")}>
        <PostProcessingSettingsApi />
      </SettingsGroup>
    </div>
  );
};
