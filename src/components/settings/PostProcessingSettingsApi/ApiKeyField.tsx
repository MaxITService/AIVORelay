import React, { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { type } from "@tauri-apps/plugin-os";

import { ApiKeyEditor, StoredApiKeyDisplay } from "../ApiKeyControls";

type LlmApiKeyFeature = "post_processing" | "ai_replace" | "voice_command";

interface ApiKeyFieldProps {
  value: string;
  onBlur: (value: string) => void | Promise<void>;
  disabled: boolean;
  placeholder?: string;
  className?: string;
  secureStorage?: {
    feature: LlmApiKeyFeature;
    providerId: string;
  };
}

export const ApiKeyField: React.FC<ApiKeyFieldProps> = React.memo(
  ({
    value,
    onBlur,
    disabled,
    placeholder,
    className = "",
    secureStorage,
  }) => {
    const { t } = useTranslation();
    const isWindows = type() === "windows";
    const [localValue, setLocalValue] = useState(value);
    const [hasSecureKey, setHasSecureKey] = useState(false);
    const [hasSecureStatusLoaded, setHasSecureStatusLoaded] = useState(false);
    const [isEditingSecureKey, setIsEditingSecureKey] = useState(false);
    const [isCheckingSecureStatus, setIsCheckingSecureStatus] = useState(false);

    const secureFeature = secureStorage?.feature;
    const secureProviderId = secureStorage?.providerId?.trim() ?? "";
    const shouldCheckSecureStorage =
      isWindows &&
      !!secureFeature &&
      secureProviderId.length > 0 &&
      value.trim().length === 0;

    const containerClassName = `flex-1 min-w-[320px] ${className}`.trim();

    // Sync with prop changes
    useEffect(() => {
      setLocalValue(value);
    }, [value]);

    const loadSecureStatus = useCallback(async () => {
      if (!shouldCheckSecureStorage || !secureFeature || !secureProviderId) {
        setHasSecureKey(false);
        setHasSecureStatusLoaded(true);
        return;
      }

      setIsCheckingSecureStatus(true);
      try {
        const hasStoredKey = await invoke<boolean>("llm_has_stored_api_key", {
          feature: secureFeature,
          providerId: secureProviderId,
        });
        setHasSecureKey(Boolean(hasStoredKey));
      } catch (error) {
        console.error("Failed to check secure API key status:", error);
        setHasSecureKey(false);
      } finally {
        setIsCheckingSecureStatus(false);
        setHasSecureStatusLoaded(true);
      }
    }, [secureFeature, secureProviderId, shouldCheckSecureStorage]);

    useEffect(() => {
      void loadSecureStatus();
    }, [loadSecureStatus]);

    useEffect(() => {
      if (!hasSecureStatusLoaded || !shouldCheckSecureStorage) {
        return;
      }
      if (!hasSecureKey) {
        setIsEditingSecureKey(false);
      }
    }, [hasSecureKey, hasSecureStatusLoaded, shouldCheckSecureStorage]);

    const handleStartReplace = () => {
      setLocalValue("");
      setIsEditingSecureKey(true);
    };

    const handleCancelReplace = () => {
      setLocalValue("");
      setIsEditingSecureKey(false);
    };

    const handleSaveReplace = async () => {
      const trimmed = localValue.trim();
      if (!trimmed || disabled || isCheckingSecureStatus) return;

      await Promise.resolve(onBlur(trimmed));
      setLocalValue("");
      await loadSecureStatus();
      setIsEditingSecureKey(false);
    };

    const handleClearStoredKey = async () => {
      if (disabled || isCheckingSecureStatus) return;

      await Promise.resolve(onBlur(""));
      setLocalValue("");
      setIsEditingSecureKey(false);
      await loadSecureStatus();
    };

    const handleSaveLocalValue = async () => {
      if (disabled || localValue === value) return;

      await Promise.resolve(onBlur(localValue));
      if (shouldCheckSecureStorage) {
        setLocalValue("");
        await loadSecureStatus();
      }
    };

    const showStoredKeyState =
      shouldCheckSecureStorage &&
      hasSecureStatusLoaded &&
      hasSecureKey &&
      !isEditingSecureKey;

    if (showStoredKeyState) {
      return (
        <div className={`flex flex-col gap-2 ${containerClassName}`}>
          <StoredApiKeyDisplay
            disabled={disabled}
            loading={isCheckingSecureStatus}
            onDelete={handleClearStoredKey}
            onReplace={handleStartReplace}
          />
        </div>
      );
    }

    const showReplaceEditor =
      shouldCheckSecureStorage && hasSecureKey && isEditingSecureKey;

    if (showReplaceEditor) {
      return (
        <div className={`flex flex-col gap-2 ${containerClassName}`}>
          <ApiKeyEditor
            disabled={disabled}
            loading={isCheckingSecureStatus}
            value={localValue}
            onChange={setLocalValue}
            onSave={handleSaveReplace}
            onCancel={handleCancelReplace}
            placeholder={placeholder}
            showCancel
            hint={t("settings.advanced.remoteStt.apiKey.replaceHint")}
          />
        </div>
      );
    }

    return (
      <div className={`flex flex-col gap-2 ${containerClassName}`}>
        <ApiKeyEditor
          disabled={disabled}
          loading={isCheckingSecureStatus}
          value={localValue}
          onBlur={() => {
            void handleSaveLocalValue();
          }}
          onChange={setLocalValue}
          onSave={handleSaveLocalValue}
          placeholder={placeholder}
          saveDisabled={localValue === value}
          hint={
            shouldCheckSecureStorage && hasSecureStatusLoaded && !hasSecureKey
              ? t("settings.advanced.remoteStt.apiKey.statusMissing")
              : undefined
          }
        />
      </div>
    );
  },
);

ApiKeyField.displayName = "ApiKeyField";
