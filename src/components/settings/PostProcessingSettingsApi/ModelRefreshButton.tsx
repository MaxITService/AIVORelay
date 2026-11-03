import React from "react";
import { RefreshCcw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useSettingsStore } from "@/stores/settingsStore";
import { ResetButton } from "../../ui/ResetButton";
import { Tooltip } from "../../ui/Tooltip";
import "./ModelRefreshButton.css";

interface ModelRefreshButtonProps {
  providerId: string;
  onClick: () => void;
  isFetching?: boolean;
  disabled?: boolean;
  className?: string;
}

export const ModelRefreshButton: React.FC<ModelRefreshButtonProps> = ({
  providerId, onClick, isFetching = false, disabled = false, className = "",
}) => {
  const { t } = useTranslation();
  // A manually entered model is not a downloaded server list. An empty
  // successful response still counts as loaded; invalidation removes the key.
  const loaded = useSettingsStore(state =>
    Object.prototype.hasOwnProperty.call(state.postProcessModelOptions, providerId));
  const fetching = useSettingsStore(state =>
    Boolean(state.isUpdating[`post_process_models_fetch:${providerId}`] ||
      state.isUpdating[`llm_models_fetch:post_processing:${providerId}`] ||
      state.isUpdating[`llm_models_fetch:ai_replace:${providerId}`] ||
      state.isUpdating[`llm_models_fetch:voice_command:${providerId}`]));
  const busy = isFetching || fetching;
  const unavailable = disabled || !providerId || providerId === "apple_intelligence";
  const needsFetch = !loaded && !busy && !unavailable;
  const description = t(`settings.postProcessing.api.model.${
    busy ? "fetchingModelsHint" : loaded ? "refreshModelsHint" : "fetchModelsHint"}`);

  return (
    <Tooltip content={description} className="inline-flex shrink-0">
      <ResetButton
        onClick={onClick}
        disabled={unavailable || busy}
        ariaLabel={description}
        className={`flex h-10 w-10 items-center justify-center ${className}`}
      >
        <RefreshCcw
          aria-hidden="true"
          className={`h-4 w-4 ${busy ? "animate-spin" : needsFetch ? "model-refresh-needs-fetch" : ""}`}
        />
      </ResetButton>
    </Tooltip>
  );
};
