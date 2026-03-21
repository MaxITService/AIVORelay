import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
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

const ERROR_OVERLAY_CATEGORIES = [
  { value: "Auth", label: "Auth" },
  { value: "RateLimited", label: "Rate Limited" },
  { value: "Billing", label: "Billing" },
  { value: "BadRequest", label: "Bad Request" },
  { value: "TlsCertificate", label: "TLS Certificate" },
  { value: "TlsHandshake", label: "TLS Handshake" },
  { value: "Timeout", label: "Timeout" },
  { value: "NetworkError", label: "Network Error" },
  { value: "ServerError", label: "Server Error" },
  { value: "ParseError", label: "Parse Error" },
  { value: "ExtensionOffline", label: "Extension Offline" },
  { value: "MicrophoneUnavailable", label: "Mic Unavailable" },
  { value: "Unknown", label: "Unknown" },
] as const;

export const ShowOverlay: React.FC<ShowOverlayProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating, settings } = useSettings();
    const [errorOverlayAutoHideInput, setErrorOverlayAutoHideInput] =
      useState("2000");
    const [selectedErrorCategory, setSelectedErrorCategory] = useState("Auth");

    const overlayOptions = [
      { value: "none", label: t("settings.advanced.overlay.options.none") },
      { value: "bottom", label: t("settings.advanced.overlay.options.bottom") },
      { value: "top", label: t("settings.advanced.overlay.options.top") },
    ];

    const selectedPosition = (getSetting("overlay_position") ||
      "bottom") as OverlayPosition;
    const autoPositionAllowReservedAreas =
      Boolean((settings as any)?.auto_position_allow_reserved_areas ?? false);
    const errorFeedbackEnabled =
      (getSetting("error_feedback_enabled" as any) ?? true) === true;
    const recordingOverlayShowDragGrip = Boolean(
      (settings as any)?.recording_overlay_show_drag_grip ?? false,
    );
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
          checked={autoPositionAllowReservedAreas}
          onChange={(enabled) =>
            void updateSetting(
              "auto_position_allow_reserved_areas" as any,
              enabled as any,
            )
          }
          isUpdating={isUpdating("auto_position_allow_reserved_areas")}
          label={t(
            "settings.advanced.overlay.allowReservedAreas.label",
            "Allow Auto-Positioning in Reserved Areas",
          )}
          description={t(
            "settings.advanced.overlay.allowReservedAreas.description",
            "Let auto-placed overlay, preview, confirmation, and voice button windows use taskbar or docked-bar space. Manual dragging can still place them anywhere.",
          )}
          descriptionMode={descriptionMode}
          grouped={grouped}
        />

        <ToggleSwitch
          checked={recordingOverlayShowDragGrip}
          onChange={(enabled) =>
            void updateSetting(
              "recording_overlay_show_drag_grip" as any,
              enabled as any,
            )
          }
          isUpdating={isUpdating("recording_overlay_show_drag_grip")}
          label={t(
            "settings.advanced.overlay.dragGrip.label",
            "Show Recording Overlay Drag Grip",
          )}
          description={t(
            "settings.advanced.overlay.dragGrip.description",
            "Show a small top handle on hover so you can drag the recording overlay during a session without leaving the grip visible all the time.",
          )}
          descriptionMode={descriptionMode}
          grouped={grouped}
        />
        <p className="px-6 text-xs text-text/60 -mt-2 mb-2">
          {t(
            "settings.advanced.overlay.dragGrip.help",
            "Tip: move the pointer over the overlay to reveal the grip, then drag from the top center.",
          )}
        </p>

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
        <p className="px-6 text-xs text-red-400 -mt-2 mb-2">
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

        <SettingContainer
          title={t(
            "settings.userInterface.recordingOverlay.errorPreview.title",
            "Overlay Error Test",
          )}
          description={t(
            "settings.userInterface.recordingOverlay.errorPreview.description",
            "Show a sample error overlay so you can preview each built-in error state.",
          )}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <div className="flex items-center gap-2">
            <select
              value={selectedErrorCategory}
              onChange={(event) => setSelectedErrorCategory(event.target.value)}
              className="px-2 py-1.5 bg-[#2b2b2b] border border-[#3c3c3c] rounded-lg text-xs text-gray-200 font-medium transition-colors appearance-none cursor-pointer"
            >
              {ERROR_OVERLAY_CATEGORIES.map((category) => (
                <option key={category.value} value={category.value}>
                  {category.label}
                </option>
              ))}
            </select>
            <button
              type="button"
              onClick={() => {
                void invoke("debug_show_error_overlay", {
                  category: selectedErrorCategory,
                });
              }}
              className="px-3 py-1.5 bg-[#2b2b2b] hover:bg-[#3c3c3c] border border-[#3c3c3c] rounded-lg text-xs text-gray-200 font-medium transition-colors"
            >
              {t(
                "settings.userInterface.recordingOverlay.errorPreview.button",
                "Show Error Overlay",
              )}
            </button>
          </div>
        </SettingContainer>
      </>
    );
  },
);
