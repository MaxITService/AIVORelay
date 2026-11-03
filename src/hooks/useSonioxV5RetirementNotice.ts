import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { commands } from "@/bindings";
import { sessionToast as toast } from "@/lib/sessionToast";

// Soniox removes the v5 models on December 15, 2026.
const SONIOX_V5_REPLACEMENTS: Record<string, string> = {
  "stt-rt-v5": "stt-rt-v6",
  "stt-async-v5": "stt-async-v6",
};

/**
 * Shows one startup toast when Soniox is the active provider and its saved
 * model is a retiring v5 model. The model only changes when the user clicks
 * the toast action; any failure is logged and leaves settings untouched.
 */
export const useSonioxV5RetirementNotice = (
  enabled: boolean,
  refreshSettings: () => Promise<void>,
) => {
  const { t } = useTranslation();
  const checked = useRef(false);

  useEffect(() => {
    if (!enabled || checked.current) return;
    checked.current = true;

    const upgrade = async (currentModel: string, targetModel: string) => {
      try {
        const latest = await commands.getAppSettings();
        // Do not overwrite a model the user changed after the toast appeared.
        if (latest.status !== "ok" || (latest.data.soniox_model ?? "").trim() !== currentModel) {
          await refreshSettings();
          return;
        }
        const result = await commands.changeSonioxModelSetting(targetModel);
        if (result.status === "error") throw new Error(result.error);
        await refreshSettings();
        toast.success(
          t("settings.advanced.soniox.v5Retirement.success", {
            defaultValue: "Soniox model updated to {{model}}.",
            model: targetModel,
          }),
        );
      } catch (error) {
        console.error("Failed to update the Soniox model:", error);
        toast.error(
          t("settings.advanced.soniox.v5Retirement.failed", {
            defaultValue: "Could not update the Soniox model. Change it in Speech-to-Text settings.",
          }),
        );
      }
    };

    void (async () => {
      try {
        const result = await commands.getAppSettings();
        if (result.status !== "ok") return;
        const settings = result.data;
        if (settings.transcription_provider !== "remote_soniox") return;
        const currentModel = (settings.soniox_model ?? "").trim();
        const targetModel = SONIOX_V5_REPLACEMENTS[currentModel];
        if (!targetModel) return;

        toast.warning(
          t("settings.advanced.soniox.v5Retirement.title", {
            defaultValue: "Soniox {{model}} will stop working on December 15, 2026",
            model: currentModel,
          }),
          {
            duration: 30000,
            description: t("settings.advanced.soniox.v5Retirement.description", {
              defaultValue: "Soniox is retiring v5. Switch to {{target}} to keep transcription working.",
              target: targetModel,
            }),
            action: {
              label: t("settings.advanced.soniox.v5Retirement.action", {
                defaultValue: "Update to {{target}}",
                target: targetModel,
              }),
              onClick: () => void upgrade(currentModel, targetModel),
            },
          },
        );
      } catch (error) {
        console.error("Failed to check the Soniox model version:", error);
      }
    })();
  }, [enabled, refreshSettings, t]);
};
