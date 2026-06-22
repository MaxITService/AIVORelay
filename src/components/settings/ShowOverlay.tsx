import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import { Input } from "../ui/Input";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import type { OverlayPosition } from "@/bindings";

const MAX_ERROR_OVERLAY_AUTO_HIDE_MS = 100_000;
const MAX_CUSTOM_COORDINATE_PX = 100_000;

type RecordingOverlayPositionValue = OverlayPosition | "custom";

type AppliedCustomPosition = {
  x_px: number;
  y_px: number;
};

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
    const {
      getSetting,
      updateSetting,
      isUpdating,
      settings,
      refreshSettings,
    } = useSettings();
    const [errorOverlayAutoHideInput, setErrorOverlayAutoHideInput] =
      useState("3500");
    const [selectedErrorCategory, setSelectedErrorCategory] = useState("Auth");
    const [isEditingCustomPosition, setIsEditingCustomPosition] =
      useState(false);
    const [isApplyingCustomPosition, setIsApplyingCustomPosition] =
      useState(false);
    const [customXDraft, setCustomXDraft] = useState("0");
    const [customYDraft, setCustomYDraft] = useState("0");
    const [customPositionError, setCustomPositionError] = useState<
      string | null
    >(null);

    const storedPosition = (getSetting("overlay_position") ||
      "bottom_left") as OverlayPosition;
    const normalizedAutomaticPosition: OverlayPosition =
      storedPosition === "none" ? "bottom_left" : storedPosition;
    const recordingOverlayEnabled = Boolean(
      (settings as any)?.recording_overlay_enabled ??
        storedPosition !== "none",
    );
    const useManualPosition = Boolean(
      (settings as any)?.recording_overlay_use_manual_position ?? false,
    );
    const hasSavedCustomPosition = Boolean(
      (settings as any)?.recording_overlay_has_saved_custom_position ??
        (useManualPosition ||
          Boolean(
            (settings as any)
              ?.recording_overlay_manual_position_uses_physical_px,
          )),
    );
    const selectedPosition: RecordingOverlayPositionValue =
      hasSavedCustomPosition && useManualPosition
        ? "custom"
        : normalizedAutomaticPosition;
    const persistedCustomX = Number(
      (settings as any)?.recording_overlay_custom_x_px ?? 0,
    );
    const persistedCustomY = Number(
      (settings as any)?.recording_overlay_custom_y_px ?? 0,
    );
    const overlayOptions = [
      {
        value: "top",
        label: t("settings.advanced.overlay.options.top", "Top"),
      },
      {
        value: "top_left",
        label: t("settings.advanced.overlay.options.topLeft", "Top Left"),
      },
      {
        value: "top_right",
        label: t("settings.advanced.overlay.options.topRight", "Top Right"),
      },
      {
        value: "bottom",
        label: t("settings.advanced.overlay.options.bottom", "Bottom"),
      },
      {
        value: "bottom_left",
        label: t("settings.advanced.overlay.options.bottomLeft", "Bottom Left"),
      },
      {
        value: "bottom_right",
        label: t(
          "settings.advanced.overlay.options.bottomRight",
          "Bottom Right",
        ),
      },
      ...(hasSavedCustomPosition
        ? [
            {
              value: "custom",
              label: t(
                "settings.advanced.overlay.options.custom",
                "Custom",
              ),
            },
          ]
        : []),
    ];
    const autoPositionAllowReservedAreas =
      Boolean((settings as any)?.auto_position_allow_reserved_areas ?? false);
    const errorFeedbackEnabled =
      (getSetting("error_feedback_enabled" as any) ?? true) === true;
    const recordingOverlayShowDragGrip = Boolean(
      (settings as any)?.recording_overlay_show_drag_grip ?? true,
    );
    const errorOverlayAutoHideMs = Number(
      settings?.error_overlay_auto_hide_ms ?? 3500,
    );
    const isErrorOverlayAutoHideUpdating = isUpdating("error_overlay_auto_hide_ms");

    useEffect(() => {
      setErrorOverlayAutoHideInput(String(errorOverlayAutoHideMs));
    }, [errorOverlayAutoHideMs]);

    useEffect(() => {
      if (isEditingCustomPosition) {
        return;
      }
      setCustomXDraft(String(persistedCustomX));
      setCustomYDraft(String(persistedCustomY));
      setCustomPositionError(null);
    }, [isEditingCustomPosition, persistedCustomX, persistedCustomY]);

    useEffect(() => {
      let disposed = false;
      let unlisten: (() => void) | undefined;
      void listen("recording-overlay-position-settings-changed", () => {
        void refreshSettings();
      })
        .then((cleanup) => {
          if (disposed) {
            cleanup();
          } else {
            unlisten = cleanup;
          }
        })
        .catch((error) => {
          console.error(
            "Failed to listen for recording overlay position changes:",
            error,
          );
        });

      return () => {
        disposed = true;
        unlisten?.();
      };
    }, [refreshSettings]);

    const parsedInputMs = Number.parseInt(errorOverlayAutoHideInput, 10);
    const hasValidInput = Number.isFinite(parsedInputMs) && parsedInputMs >= 0;
    const normalizedInputMs = hasValidInput
      ? Math.min(Math.round(parsedInputMs), MAX_ERROR_OVERLAY_AUTO_HIDE_MS)
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

    const selectPosition = async (value: string) => {
      await updateSetting(
        "overlay_position",
        value as RecordingOverlayPositionValue as OverlayPosition,
      );
      await refreshSettings();
    };

    const cancelCustomPositionEdit = () => {
      setCustomXDraft(String(persistedCustomX));
      setCustomYDraft(String(persistedCustomY));
      setCustomPositionError(null);
      setIsEditingCustomPosition(false);
    };

    const parseCustomCoordinate = (value: string): number | null => {
      const trimmed = value.trim();
      if (!/^[+-]?\d+$/.test(trimmed)) {
        return null;
      }
      const parsed = Number(trimmed);
      return Number.isFinite(parsed) && Number.isInteger(parsed) ? parsed : null;
    };

    const applyCustomPosition = async () => {
      const x = parseCustomCoordinate(customXDraft);
      const y = parseCustomCoordinate(customYDraft);
      if (x === null || y === null) {
        setCustomPositionError(
          t(
            "settings.advanced.overlay.customPosition.integerError",
            "Enter a whole number for both X and Y.",
          ),
        );
        return;
      }
      if (
        Math.abs(x) > MAX_CUSTOM_COORDINATE_PX ||
        Math.abs(y) > MAX_CUSTOM_COORDINATE_PX
      ) {
        setCustomPositionError(
          t(
            "settings.advanced.overlay.customPosition.rangeError",
            "X and Y must be between -100000 and 100000.",
          ),
        );
        return;
      }

      setIsApplyingCustomPosition(true);
      setCustomPositionError(null);
      try {
        const position = await invoke<AppliedCustomPosition>(
          "apply_recording_overlay_custom_position",
          { xPx: x, yPx: y },
        );
        setCustomXDraft(String(position.x_px));
        setCustomYDraft(String(position.y_px));
        await refreshSettings();
        setIsEditingCustomPosition(false);
      } catch (error) {
        console.error("Failed to apply recording overlay position:", error);
        setCustomPositionError(
          t(
            "settings.advanced.overlay.customPosition.applyError",
            "Could not apply this position. Check that a monitor is connected and try again.",
          ),
        );
      } finally {
        setIsApplyingCustomPosition(false);
      }
    };

    return (
      <>
        <ToggleSwitch
          checked={recordingOverlayEnabled}
          onChange={(enabled) =>
            void updateSetting(
              "recording_overlay_enabled" as any,
              enabled as any,
            )
          }
          isUpdating={isUpdating("recording_overlay_enabled")}
          label={t(
            "settings.advanced.overlay.visibility.label",
            "Show Recording Overlay",
          )}
          description={t(
            "settings.advanced.overlay.visibility.description",
            "Show recording, processing, error, and switch-status feedback without affecting transcription.",
          )}
          descriptionMode={descriptionMode}
          grouped={grouped}
        />

        <SettingContainer
          title={t("settings.advanced.overlay.title")}
          description={t(
            "settings.advanced.overlay.positionDescription",
            "Choose where the recording overlay appears. Corner presets keep the visible frame inside the usable work area.",
          )}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <div className="w-full space-y-3">
            <Dropdown
              options={overlayOptions}
              selectedValue={selectedPosition}
              onSelect={(value) => void selectPosition(value)}
              disabled={
                !recordingOverlayEnabled || isUpdating("overlay_position")
              }
            />

            {hasSavedCustomPosition && selectedPosition === "custom" && (
              <fieldset
                disabled={!recordingOverlayEnabled}
                className={`rounded-md border border-[#343434] bg-[#181818]/70 p-3 ${
                  recordingOverlayEnabled ? "" : "opacity-40"
                }`}
              >
                <div className="mb-2 text-xs font-medium text-[#e5e5e5]">
                  {t(
                    "settings.advanced.overlay.customPosition.title",
                    "Custom Position",
                  )}
                </div>
                <p className="mb-3 text-xs text-text/60">
                  {t(
                    "settings.advanced.overlay.customPosition.description",
                    "Global physical pixel coordinates for the visible overlay frame. Negative values are valid on monitors left of or above the primary display. Apply keeps the frame inside the nearest monitor's usable work area.",
                  )}
                </p>
                <div className="flex flex-wrap items-end gap-2">
                  <label className="flex min-w-28 flex-1 flex-col gap-1 text-xs text-text/70">
                    <span>
                      {t(
                        "settings.advanced.overlay.customPosition.xLabel",
                        "X",
                      )}
                    </span>
                    <Input
                      type="text"
                      inputMode="numeric"
                      variant="compact"
                      value={customXDraft}
                      onChange={(event) => {
                        setCustomXDraft(event.target.value);
                        setCustomPositionError(null);
                      }}
                      readOnly={
                        !isEditingCustomPosition || isApplyingCustomPosition
                      }
                      aria-label={t(
                        "settings.advanced.overlay.customPosition.xAriaLabel",
                        "Custom overlay X coordinate",
                      )}
                    />
                  </label>
                  <label className="flex min-w-28 flex-1 flex-col gap-1 text-xs text-text/70">
                    <span>
                      {t(
                        "settings.advanced.overlay.customPosition.yLabel",
                        "Y",
                      )}
                    </span>
                    <Input
                      type="text"
                      inputMode="numeric"
                      variant="compact"
                      value={customYDraft}
                      onChange={(event) => {
                        setCustomYDraft(event.target.value);
                        setCustomPositionError(null);
                      }}
                      readOnly={
                        !isEditingCustomPosition || isApplyingCustomPosition
                      }
                      aria-label={t(
                        "settings.advanced.overlay.customPosition.yAriaLabel",
                        "Custom overlay Y coordinate",
                      )}
                    />
                  </label>
                  {!isEditingCustomPosition ? (
                    <button
                      type="button"
                      onClick={() => setIsEditingCustomPosition(true)}
                      className="rounded-md border border-[#3c3c3c] bg-[#202020] px-3 py-1.5 text-xs font-medium text-[#e5e5e5] transition-colors hover:bg-[#2a2a2a]"
                    >
                      {t(
                        "settings.advanced.overlay.customPosition.edit",
                        "Edit Position",
                      )}
                    </button>
                  ) : (
                    <div className="flex gap-2">
                      <button
                        type="button"
                        onClick={() => void applyCustomPosition()}
                        disabled={isApplyingCustomPosition}
                        className="rounded-md border border-[#8040b8] bg-[#63308e] px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-[#7439a6] disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        {t(
                          "settings.advanced.overlay.customPosition.apply",
                          "Apply",
                        )}
                      </button>
                      <button
                        type="button"
                        onClick={cancelCustomPositionEdit}
                        disabled={isApplyingCustomPosition}
                        className="rounded-md border border-[#3c3c3c] bg-[#202020] px-3 py-1.5 text-xs font-medium text-[#e5e5e5] transition-colors hover:bg-[#2a2a2a] disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        {t(
                          "settings.advanced.overlay.customPosition.cancel",
                          "Cancel",
                        )}
                      </button>
                    </div>
                  )}
                </div>
                {customPositionError && (
                  <p role="alert" className="mt-2 text-xs text-red-400">
                    {customPositionError}
                  </p>
                )}
              </fieldset>
            )}
          </div>
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
            "settings.advanced.overlay.allowReservedAreas.descriptionWithSafeCorners",
            "Let overlay positions, preview, confirmation, and voice button windows use taskbar or docked-bar space. Bottom Left leaves room for the taskbar weather area.",
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
          disabled={!recordingOverlayEnabled}
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
        <p
          className={`px-6 text-xs text-red-400 -mt-2 mb-2 ${
            recordingOverlayEnabled ? "" : "opacity-40"
          }`}
        >
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
              min={0}
              max={MAX_ERROR_OVERLAY_AUTO_HIDE_MS}
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
