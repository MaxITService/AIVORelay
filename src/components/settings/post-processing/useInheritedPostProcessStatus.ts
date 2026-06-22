import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import { useSettings } from "../../../hooks/useSettings";

const APPLE_PROVIDER_ID = "apple_intelligence";

type SecureKeyStatus = {
  providerId: string;
  hasKey: boolean;
};

export type InheritedPostProcessStatus = {
  isReady: boolean;
  label: string;
  className: string;
};

export const useInheritedPostProcessStatus =
  (): InheritedPostProcessStatus => {
    const { settings } = useSettings();
    const { t } = useTranslation();
    const [secureKeyStatus, setSecureKeyStatus] =
      useState<SecureKeyStatus | null>(null);

    const providerId = settings?.post_process_provider_id?.trim() ?? "";
    const provider = useMemo(
      () =>
        settings?.post_process_providers?.find(
          (candidate) => candidate.id === providerId,
        ),
      [providerId, settings?.post_process_providers],
    );
    const model =
      settings?.post_process_models?.[providerId]?.trim() ?? "";
    const localApiKey =
      settings?.post_process_api_keys?.[providerId]?.trim() ?? "";
    const isAppleProvider = providerId === APPLE_PROVIDER_ID;

    useEffect(() => {
      let cancelled = false;

      if (!providerId || isAppleProvider || localApiKey) {
        setSecureKeyStatus(null);
        return () => {
          cancelled = true;
        };
      }

      setSecureKeyStatus((current) =>
        current?.providerId === providerId ? current : null,
      );

      void invoke<boolean>("llm_has_stored_api_key", {
        feature: "post_processing",
        providerId,
      })
        .then((hasKey) => {
          if (!cancelled) {
            setSecureKeyStatus({ providerId, hasKey: Boolean(hasKey) });
          }
        })
        .catch((error) => {
          console.error(
            "Failed to check inherited post-processing API key status:",
            error,
          );
          if (!cancelled) {
            setSecureKeyStatus({ providerId, hasKey: false });
          }
        });

      return () => {
        cancelled = true;
      };
    }, [isAppleProvider, localApiKey, providerId]);

    const hasApiKey =
      isAppleProvider ||
      Boolean(localApiKey) ||
      (secureKeyStatus?.providerId === providerId && secureKeyStatus.hasKey);
    const hasModel = isAppleProvider || Boolean(model);
    const isReady = Boolean(provider && hasModel && hasApiKey);
    const details = provider
      ? [provider.label, isAppleProvider ? "" : model].filter(Boolean).join(" · ")
      : "";
    const optionLabel = t(
      "settings.postProcessing.api.inherited.option",
      "Same as Post-Processing",
    );
    const statusLabel = isReady
      ? details
      : t("settings.postProcessing.api.inherited.notSet", "Not set");

    return {
      isReady,
      label: `${optionLabel} (${statusLabel})`,
      className: isReady ? "text-emerald-300" : "text-amber-300",
    };
  };
