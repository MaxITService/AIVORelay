import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { type as getOsType } from "@tauri-apps/plugin-os";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import type { ClipboardHandling } from "@/bindings";

interface ClipboardHandlingProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const ClipboardHandlingSetting: React.FC<ClipboardHandlingProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const [osType, setOsType] = useState<string>("unknown");

    useEffect(() => {
      setOsType(getOsType());
    }, []);

    // The whole section only makes sense when pasting actually goes through
    // the clipboard.
    const pasteMethod = (getSetting("paste_method") || "ctrl_v") as string;
    const usesClipboard = ["ctrl_v", "ctrl_shift_v", "shift_insert"].includes(
      pasteMethod,
    );

    const handling = (getSetting("clipboard_handling") ||
      "restore_plain_text") as ClipboardHandling;
    const restoreSelected = handling !== "keep_transcription";
    const historyAllowed = (getSetting("clipboard_history_allowed") ??
      true) as boolean;

    if (!usesClipboard) {
      return null;
    }

    const restoreMethodOptions = [
      {
        value: "restore_plain_text",
        label: t(
          "settings.advanced.clipboardHandling.restoreMethod.options.plainText",
        ),
      },
    ];

    if (osType === "windows") {
      restoreMethodOptions.push({
        value: "restore_advanced",
        label: t(
          "settings.advanced.clipboardHandling.restoreMethod.options.allFormats",
        ),
      });
      restoreMethodOptions.push({
        value: "restore_advanced_owned",
        label: t(
          "settings.advanced.clipboardHandling.restoreMethod.options.allFormatsOwned",
        ),
      });
    }

    const radio = (
      value: "restore" | "keep",
      labelKey: string,
      checked: boolean,
      target: ClipboardHandling,
    ) => (
      <label
        className={`inline-flex items-center gap-2 text-sm ${
          isUpdating("clipboard_handling")
            ? "cursor-not-allowed opacity-60"
            : "cursor-pointer"
        }`}
      >
        <input
          type="radio"
          name="clipboard-after-paste"
          value={value}
          checked={checked}
          disabled={isUpdating("clipboard_handling")}
          onChange={() => updateSetting("clipboard_handling", target)}
          className="accent-[#9b5de5]"
        />
        <span>{t(labelKey)}</span>
      </label>
    );

    return (
      <>
        <SettingContainer
          title={t("settings.advanced.clipboardHandling.title")}
          description={t("settings.advanced.clipboardHandling.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <div className="flex flex-col gap-1.5">
            {radio(
              "restore",
              "settings.advanced.clipboardHandling.options.restore",
              restoreSelected,
              "restore_plain_text" as ClipboardHandling,
            )}
            {radio(
              "keep",
              "settings.advanced.clipboardHandling.options.keepTranscription",
              !restoreSelected,
              "keep_transcription" as ClipboardHandling,
            )}
          </div>
        </SettingContainer>
        <SettingContainer
          title={t("settings.advanced.clipboardHandling.restoreMethod.title")}
          description={t(
            "settings.advanced.clipboardHandling.restoreMethod.description",
          )}
          descriptionMode={descriptionMode}
          grouped={grouped}
          disabled={!restoreSelected}
        >
          <Dropdown
            options={restoreMethodOptions}
            selectedValue={restoreSelected ? handling : "restore_plain_text"}
            onSelect={(value) =>
              updateSetting("clipboard_handling", value as ClipboardHandling)
            }
            disabled={!restoreSelected || isUpdating("clipboard_handling")}
          />
        </SettingContainer>
        {restoreSelected && osType === "windows" && (
          <ToggleSwitch
            label={t("settings.advanced.clipboardHandling.history.title")}
            description={t(
              "settings.advanced.clipboardHandling.history.description",
            )}
            descriptionMode={descriptionMode}
            grouped={grouped}
            checked={historyAllowed}
            onChange={(value) =>
              updateSetting("clipboard_history_allowed", value)
            }
            isUpdating={isUpdating("clipboard_history_allowed")}
          />
        )}
      </>
    );
  });
