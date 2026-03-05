import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import { Input } from "../ui/Input";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import type { OverlayPosition } from "@/bindings";

interface ShowOverlayProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const ShowOverlay: React.FC<ShowOverlayProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating, settings } = useSettings();
    const [errorOverlayAutoHideInput, setErrorOverlayAutoHideInput] =
      useState("2000");

    const overlayOptions = [
      { value: "none", label: t("settings.advanced.overlay.options.none") },
      { value: "bottom", label: t("settings.advanced.overlay.options.bottom") },
      { value: "top", label: t("settings.advanced.overlay.options.top") },
    ];

    const selectedPosition = (getSetting("overlay_position") ||
      "bottom") as OverlayPosition;
    const errorFeedbackEnabled =
      (getSetting("error_feedback_enabled" as any) ?? true) === true;
    const errorOverlayAutoHideMs = Number(
      settings?.error_overlay_auto_hide_ms ?? 2000,
    );
    const isErrorOverlayAutoHideUpdating = isUpdating("error_overlay_auto_hide_ms");

    useEffect(() => {
      setErrorOverlayAutoHideInput(String(errorOverlayAutoHideMs));
    }, [errorOverlayAutoHideMs]);

    const parsedInputMs = Number.parseInt(errorOverlayAutoHideInput, 10);
    const hasValidInput = Number.isFinite(parsedInputMs) && parsedInputMs >= 0;
    const normalizedInputMs = hasValidInput
      ? Math.round(parsedInputMs)
      : errorOverlayAutoHideMs;
    const hasPendingErrorOverlayAutoHideChange =
      normalizedInputMs !== errorOverlayAutoHideMs;

    const applyErrorOverlayDuration = () => {
      if (!hasValidInput) {
        return;
      }
      void updateSetting(
        "error_overlay_auto_hide_ms",
        normalizedInputMs,
      );
    };

    return (
      <>
        <SettingContainer
          title={t("settings.advanced.overlay.title")}
          description={t("settings.advanced.overlay.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <Dropdown
            options={overlayOptions}
            selectedValue={selectedPosition}
            onSelect={(value) =>
              updateSetting("overlay_position", value as OverlayPosition)
            }
            disabled={isUpdating("overlay_position")}
          />
        </SettingContainer>

        <ToggleSwitch
          checked={errorFeedbackEnabled}
          onChange={(enabled) =>
            void updateSetting("error_feedback_enabled" as any, enabled as any)
          }
          isUpdating={isUpdating("error_feedback_enabled")}
          label={t(
            "settings.advanced.overlay.errorVisibility.label",
            "Show Error Overlay",
          )}
          description={t(
            "settings.advanced.overlay.errorVisibility.description",
            "Show runtime errors in the recording overlay.",
          )}
          descriptionMode={descriptionMode}
          grouped={grouped}
        />
        <p className="text-xs text-red-400 -mt-2 mb-2">
          {t(
            "settings.advanced.overlay.errorVisibility.danger",
            "Dangerous: disabling overlay errors can hide broken setup and failed transcriptions.",
          )}
        </p>

        <SettingContainer
          title={t("settings.advanced.overlay.errorDuration.title")}
          description={t("settings.advanced.overlay.errorDuration.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <div className="flex items-center gap-2">
            <Input
              type="number"
              variant="compact"
              step={50}
              value={errorOverlayAutoHideInput}
              onChange={(event) => setErrorOverlayAutoHideInput(event.target.value)}
              className="w-28 text-right"
              disabled={isErrorOverlayAutoHideUpdating}
              aria-label={t("settings.advanced.overlay.errorDuration.title")}
            />
            <span className="text-xs text-text/60 min-w-14">
              {t("settings.advanced.overlay.errorDuration.unitMs", "ms")}
            </span>
            <button
              type="button"
              onClick={applyErrorOverlayDuration}
              disabled={
                isErrorOverlayAutoHideUpdating ||
                !hasValidInput ||
                !hasPendingErrorOverlayAutoHideChange
              }
              className="px-3 py-1.5 bg-[#2b2b2b] hover:bg-[#3c3c3c] border border-[#3c3c3c] rounded-lg text-xs text-gray-200 font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {t("settings.advanced.overlay.errorDuration.apply", "Apply")}
            </button>
          </div>
        </SettingContainer>
      </>
    );
  },
);
