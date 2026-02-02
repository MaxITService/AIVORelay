import { useCallback, useMemo } from "react";
import { useSettings } from "../../../hooks/useSettings";
import type { PostProcessProvider } from "@/bindings";
import type { DropdownOption } from "../../ui/Dropdown";

export interface ModelOption {
  value: string;
  label: string;
}

type VoiceCommandProviderState = {
  /** Whether "use same as post-processing" is selected */
  useSameAsPostProcess: boolean;
  providerOptions: DropdownOption[];
  selectedProviderId: string;
  selectedProvider: PostProcessProvider | undefined;
  isCustomProvider: boolean;
  isAppleProvider: boolean;
  baseUrl: string;
  apiKey: string;
  handleApiKeyChange: (value: string) => void;
  isApiKeyUpdating: boolean;
  model: string;
  handleModelChange: (value: string) => void;
  modelOptions: ModelOption[];
  isModelUpdating: boolean;
  isFetchingModels: boolean;
  handleProviderSelect: (providerId: string | null) => void;
  handleModelSelect: (value: string) => void;
  handleModelCreate: (value: string) => void;
  handleRefreshModels: () => void;
};

const APPLE_PROVIDER_ID = "apple_intelligence";
const SAME_AS_POST_PROCESS_VALUE = "__same_as_post_process__";

export const useVoiceCommandProviderState = (): VoiceCommandProviderState => {
  const {
    settings,
    isUpdating,
    postProcessModelOptions,
    fetchLlmModels,
    setVoiceCommandProvider,
    updateVoiceCommandApiKey,
    updateVoiceCommandModel,
  } = useSettings();

  const providers = settings?.post_process_providers || [];

  // If voice_command_provider_id is null/undefined, use post-processing provider
  const useSameAsPostProcess = !settings?.voice_command_provider_id;

  const effectiveProviderId = useMemo(() => {
    if (settings?.voice_command_provider_id) {
      return settings.voice_command_provider_id;
    }
    return settings?.post_process_provider_id || providers[0]?.id || "openai";
  }, [
    settings?.voice_command_provider_id,
    settings?.post_process_provider_id,
    providers,
  ]);

  const selectedProvider = useMemo(() => {
    return (
      providers.find((provider) => provider.id === effectiveProviderId) ||
      providers[0]
    );
  }, [providers, effectiveProviderId]);

  const isAppleProvider = selectedProvider?.id === APPLE_PROVIDER_ID;
  const isCustomProvider = selectedProvider?.id === "custom";

  // Use Voice Command-specific settings, falling back to post-processing
  const baseUrl = selectedProvider?.base_url ?? "";
  const apiKey = useMemo(() => {
    const voiceCommandKey =
      settings?.voice_command_api_keys?.[effectiveProviderId] ?? "";
    if (voiceCommandKey) return voiceCommandKey;
    return settings?.post_process_api_keys?.[effectiveProviderId] ?? "";
  }, [
    settings?.voice_command_api_keys,
    settings?.post_process_api_keys,
    effectiveProviderId,
  ]);

  const model = useMemo(() => {
    const voiceCommandModel =
      settings?.voice_command_models?.[effectiveProviderId] ?? "";
    if (voiceCommandModel) return voiceCommandModel;
    return settings?.post_process_models?.[effectiveProviderId] ?? "";
  }, [
    settings?.voice_command_models,
    settings?.post_process_models,
    effectiveProviderId,
  ]);

  // Include "Same as Post-Processing" option
  const providerOptions = useMemo<DropdownOption[]>(() => {
    const options: DropdownOption[] = [
      {
        value: SAME_AS_POST_PROCESS_VALUE,
        label: "Same as Post-Processing",
      },
    ];
    providers.forEach((provider) => {
      options.push({
        value: provider.id,
        label: provider.label,
      });
    });
    return options;
  }, [providers]);

  const handleProviderSelect = useCallback(
    async (providerId: string | null) => {
      // If "Same as Post-Processing" is selected, set to null
      const newProviderId =
        providerId === SAME_AS_POST_PROCESS_VALUE ? null : providerId;

      try {
        await setVoiceCommandProvider(newProviderId);
      } catch (error) {
        console.error("Failed to set Voice Command provider:", error);
      }
    },
    [setVoiceCommandProvider]
  );

  const handleApiKeyChange = useCallback(
    async (value: string) => {
      const trimmed = value.trim();
      try {
        await updateVoiceCommandApiKey(effectiveProviderId, trimmed);
      } catch (error) {
        console.error("Failed to update Voice Command API key:", error);
      }
    },
    [effectiveProviderId, updateVoiceCommandApiKey]
  );

  const handleModelChange = useCallback(
    async (value: string) => {
      const trimmed = value.trim();
      try {
        await updateVoiceCommandModel(effectiveProviderId, trimmed);
      } catch (error) {
        console.error("Failed to update Voice Command model:", error);
      }
    },
    [effectiveProviderId, updateVoiceCommandModel]
  );

  const handleModelSelect = useCallback(
    (value: string) => {
      void handleModelChange(value.trim());
    },
    [handleModelChange]
  );

  const handleModelCreate = useCallback(
    (value: string) => {
      void handleModelChange(value);
    },
    [handleModelChange]
  );

  const handleRefreshModels = useCallback(() => {
    if (isAppleProvider) return;
    // Use voice_command feature when available, fallback to post_processing for now
    void fetchLlmModels("post_processing");
  }, [fetchLlmModels, isAppleProvider]);

  const availableModelsRaw =
    postProcessModelOptions[effectiveProviderId] || [];

  const modelOptions = useMemo<ModelOption[]>(() => {
    const seen = new Set<string>();
    const options: ModelOption[] = [];

    const upsert = (value: string | null | undefined) => {
      const trimmed = value?.trim();
      if (!trimmed || seen.has(trimmed)) return;
      seen.add(trimmed);
      options.push({ value: trimmed, label: trimmed });
    };

    for (const candidate of availableModelsRaw) {
      upsert(candidate);
    }
    upsert(model);

    return options;
  }, [availableModelsRaw, model]);

  const isApiKeyUpdating = isUpdating(
    `voice_command_api_key:${effectiveProviderId}`
  );
  const isModelUpdating = isUpdating(
    `voice_command_model:${effectiveProviderId}`
  );
  const isFetchingModels = isUpdating(
    `llm_models_fetch:post_processing:${effectiveProviderId}`
  );

  // For the dropdown, represent "same as post-processing" selection
  const selectedProviderId = useSameAsPostProcess
    ? SAME_AS_POST_PROCESS_VALUE
    : effectiveProviderId;

  return {
    useSameAsPostProcess,
    providerOptions,
    selectedProviderId,
    selectedProvider,
    isCustomProvider,
    isAppleProvider,
    baseUrl,
    apiKey,
    handleApiKeyChange,
    isApiKeyUpdating,
    model,
    handleModelChange,
    modelOptions,
    isModelUpdating,
    isFetchingModels,
    handleProviderSelect,
    handleModelSelect,
    handleModelCreate,
    handleRefreshModels,
  };
};
