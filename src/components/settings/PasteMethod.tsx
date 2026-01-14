import React, { useEffect, useState } from "react";
import { useTranslation, Trans } from "react-i18next";
import { type as getOsType } from "@tauri-apps/plugin-os";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { TellMeMore } from "../ui/TellMeMore";
import { useSettings } from "../../hooks/useSettings";
import type { PasteMethod } from "@/bindings";

interface PasteMethodProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const PasteMethodSetting: React.FC<PasteMethodProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const [osType, setOsType] = useState<string>("unknown");

    const getPasteMethodOptions = (osType: string) => {
      const mod = osType === "macos" ? "Cmd" : "Ctrl";

      const options = [
        {
          value: "ctrl_v",
          label: t("settings.advanced.pasteMethod.options.clipboard", {
            modifier: mod,
          }),
        },
        {
          value: "direct",
          label: t("settings.advanced.pasteMethod.options.direct"),
        },
        {
          value: "none",
          label: t("settings.advanced.pasteMethod.options.none"),
        },
      ];

      // Add Shift+Insert and Ctrl+Shift+V options for Windows and Linux only
      if (osType === "windows" || osType === "linux") {
        options.push(
          {
            value: "ctrl_shift_v",
            label: t(
              "settings.advanced.pasteMethod.options.clipboardCtrlShiftV",
            ),
          },
          {
            value: "shift_insert",
            label: t(
              "settings.advanced.pasteMethod.options.clipboardShiftInsert",
            ),
          },
        );
      }

      return options;
    };

    useEffect(() => {
      setOsType(getOsType());
    }, []);

    const selectedMethod = (getSetting("paste_method") ||
      "ctrl_v") as PasteMethod;
    const convertLfToCrlf = (getSetting("convert_lf_to_crlf" as any) ?? true) as boolean;

    const pasteMethodOptions = getPasteMethodOptions(osType);

    // Show CRLF toggle for clipboard-based paste methods on Windows
    const isClipboardMethod =
      selectedMethod === "ctrl_v" ||
      selectedMethod === "ctrl_shift_v" ||
      selectedMethod === "shift_insert";
    const showCrlfToggle = osType === "windows" && isClipboardMethod;

    return (
      <>
        <SettingContainer
          title={t("settings.advanced.pasteMethod.title")}
          description={t("settings.advanced.pasteMethod.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
          tooltipPosition="bottom"
        >
          <Dropdown
            options={pasteMethodOptions}
            selectedValue={selectedMethod}
            onSelect={(value) =>
              updateSetting("paste_method", value as PasteMethod)
            }
            disabled={isUpdating("paste_method")}
          />
        </SettingContainer>

        {showCrlfToggle && (
          <ToggleSwitch
            checked={convertLfToCrlf}
            onChange={(enabled) => updateSetting("convert_lf_to_crlf" as any, enabled)}
            isUpdating={isUpdating("convert_lf_to_crlf")}
            label={t("settings.advanced.pasteMethod.convertLfToCrlf.label")}
            description={t("settings.advanced.pasteMethod.convertLfToCrlf.description")}
            descriptionMode={descriptionMode}
            grouped={grouped}
          />
        )}

        <TellMeMore title={t("settings.advanced.pasteMethod.tellMeMore.title")}>
          <div className="space-y-3">
            <p className="mb-2">
              <strong>{t("settings.advanced.pasteMethod.tellMeMore.headline")}</strong>
            </p>
            <p className="mb-2">
              {t("settings.advanced.pasteMethod.tellMeMore.intro")}
            </p>

            <div className="space-y-2 ml-2">
              <p>
                <strong>{t("settings.advanced.pasteMethod.tellMeMore.ctrlV.title")}</strong>{" "}
                {t("settings.advanced.pasteMethod.tellMeMore.ctrlV.description")}
              </p>
              <p>
                <strong>{t("settings.advanced.pasteMethod.tellMeMore.direct.title")}</strong>{" "}
                {t("settings.advanced.pasteMethod.tellMeMore.direct.description")}
              </p>
              <p>
                <strong>{t("settings.advanced.pasteMethod.tellMeMore.none.title")}</strong>{" "}
                {t("settings.advanced.pasteMethod.tellMeMore.none.description")}
              </p>
            </div>

            {osType === "windows" && (
              <div className="mt-3 p-2 bg-mid-gray/10 rounded border border-mid-gray/20">
                <p>
                  <strong>{t("settings.advanced.pasteMethod.tellMeMore.crlfNote.title")}</strong>{" "}
                  {t("settings.advanced.pasteMethod.tellMeMore.crlfNote.description")}
                </p>
              </div>
            )}

            <p className="mt-3 text-text/70 text-xs">
              {t("settings.advanced.pasteMethod.tellMeMore.tip")}
            </p>
          </div>
        </TellMeMore>
      </>
    );
  },
);
