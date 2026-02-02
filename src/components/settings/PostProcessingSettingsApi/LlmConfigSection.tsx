import React from "react";
import { useTranslation } from "react-i18next";
import { RefreshCcw } from "lucide-react";

import { SettingContainer } from "../../ui/SettingContainer";
import { ResetButton } from "../../ui/ResetButton";
import { ProviderSelect } from "./ProviderSelect";
import { BaseUrlField } from "./BaseUrlField";
import { ApiKeyField } from "./ApiKeyField";
import { ModelSelect } from "./ModelSelect";
import { ExtendedThinkingSection } from "../ExtendedThinkingSection";

const DisabledNotice: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => (
  <div className="p-4 bg-mid-gray/5 rounded-lg border border-mid-gray/20">
    <p className="text-sm text-mid-gray">{children}</p>
  </div>
);

interface LlmConfigSectionProps {
  title: string;
  description: string;
  state: any; // Can be from usePostProcessProviderState or useAiReplaceProviderState
  showBaseUrl?: boolean;
  /** Setting prefix for Extended Thinking: "post_process" or "ai_replace" */
  reasoningSettingPrefix?: "post_process" | "ai_replace";
  /** Translation path for "Same as Post-Processing" notice */
  sameAsNoticeKey?: string;
  /** Optional summary badge content when using post-processing settings */
  sameAsSummary?: string;
}

export const LlmConfigSection: React.FC<LlmConfigSectionProps> = ({
  title,
  description,
  state,
  showBaseUrl = true,
  reasoningSettingPrefix,
  sameAsNoticeKey,
  sameAsSummary,
}) => {
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
              {sameAsSummary || (sameAsNoticeKey ? t(sameAsNoticeKey) : "")}
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
                value={state.model || ""}
                options={state.modelOptions}
                disabled={state.isModelUpdating}
                isLoading={state.isFetchingModels}
                placeholder={
                  state.isAppleProvider
                    ? t("settings.postProcessing.api.model.placeholderApple")
                    : (state.modelOptions?.length || 0) > 0
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
