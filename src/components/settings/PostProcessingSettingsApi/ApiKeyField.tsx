import React, { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { type } from "@tauri-apps/plugin-os";

import { Input } from "../../ui/Input";
import { Button } from "../../ui/Button";

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

    const showStoredKeyState =
      shouldCheckSecureStorage &&
      hasSecureStatusLoaded &&
      hasSecureKey &&
      !isEditingSecureKey;

    if (showStoredKeyState) {
      return (
        <div className={`flex flex-col gap-2 ${containerClassName}`}>
          <Input
            type="text"
            value="************************************************"
            readOnly
            className="w-full text-green-400"
          />
          <div className="flex items-center gap-2 text-sm text-green-400">
            <span className="inline-flex h-2 w-2 rounded-full bg-green-400" />
            <span>{t("settings.advanced.remoteStt.apiKey.statusStored")}</span>
          </div>
          <p className="text-xs text-text/60">
            {t("settings.advanced.remoteStt.apiKey.statusStoredHint")}
          </p>
          <div className="flex items-center gap-2">
            <Button
              variant="secondary"
              size="sm"
              onClick={handleStartReplace}
              disabled={disabled || isCheckingSecureStatus}
            >
              {t("settings.advanced.remoteStt.apiKey.replace")}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={handleClearStoredKey}
              disabled={disabled || isCheckingSecureStatus}
            >
              {t("settings.advanced.remoteStt.apiKey.clear")}
            </Button>
          </div>
        </div>
      );
    }

    const showReplaceEditor =
      shouldCheckSecureStorage && hasSecureKey && isEditingSecureKey;

    if (showReplaceEditor) {
      return (
        <div className={`flex flex-col gap-2 ${containerClassName}`}>
          <div className="flex items-center gap-2">
            <Input
              type="password"
              value={localValue}
              onChange={(event) => setLocalValue(event.target.value)}
              placeholder={placeholder}
              variant="compact"
              disabled={disabled || isCheckingSecureStatus}
              className="flex-1 min-w-[220px]"
            />
            <Button
              variant="secondary"
              size="sm"
              onClick={handleSaveReplace}
              disabled={
                disabled || isCheckingSecureStatus || localValue.trim().length === 0
              }
            >
              {t("settings.advanced.remoteStt.apiKey.save")}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={handleCancelReplace}
              disabled={disabled || isCheckingSecureStatus}
            >
              {t("settings.advanced.remoteStt.apiKey.cancel")}
            </Button>
          </div>
          <p className="text-xs text-text/60">
            {t("settings.advanced.remoteStt.apiKey.replaceHint")}
          </p>
        </div>
      );
    }

    return (
      <div className={containerClassName}>
        <Input
          type="password"
          value={localValue}
          onChange={(event) => setLocalValue(event.target.value)}
          onBlur={() => {
            if (localValue === value) return;
            void (async () => {
              await Promise.resolve(onBlur(localValue));
              if (shouldCheckSecureStorage) {
                setLocalValue("");
                await loadSecureStatus();
              }
            })();
          }}
          placeholder={placeholder}
          variant="compact"
          disabled={disabled}
          className="w-full"
        />
      </div>
    );
  },
);

ApiKeyField.displayName = "ApiKeyField";
