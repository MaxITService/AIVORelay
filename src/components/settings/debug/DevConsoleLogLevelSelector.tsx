import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { SettingContainer } from "../../ui/SettingContainer";
import { Dropdown, type DropdownOption } from "../../ui/Dropdown";
import type { LogLevel } from "../../../bindings";

const DEV_CONSOLE_LOG_LEVEL_OPTIONS: DropdownOption[] = [
  { value: "error", label: "Error" },
  { value: "warn", label: "Warn" },
  { value: "info", label: "Info" },
  { value: "debug", label: "Debug" },
  { value: "trace", label: "Trace" },
];

interface DevConsoleLogLevelSelectorProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

/**
 * A process-local terminal verbosity switch for `cargo tauri dev`.
 *
 * It deliberately mirrors the same `import.meta.env.DEV` check used by the
 * Dev Mode badge, while the matching Rust commands are compiled only with
 * `debug_assertions`. No setting is persisted and release builds show nothing.
 */
export const DevConsoleLogLevelSelector: React.FC<
  DevConsoleLogLevelSelectorProps
> = ({ descriptionMode = "tooltip", grouped = false }) => {
  const { t } = useTranslation();
  const [currentLevel, setCurrentLevel] = useState<LogLevel>("info");
  const [isLoading, setIsLoading] = useState(true);
  const [isUpdating, setIsUpdating] = useState(false);

  useEffect(() => {
    if (!import.meta.env.DEV) {
      return;
    }

    let mounted = true;
    void invoke<LogLevel>("get_dev_console_log_level")
      .then((level) => {
        if (mounted) {
          setCurrentLevel(level);
        }
      })
      .catch((error) => {
        console.error("Failed to read Dev Console log level:", error);
      })
      .finally(() => {
        if (mounted) {
          setIsLoading(false);
        }
      });

    return () => {
      mounted = false;
    };
  }, []);

  if (!import.meta.env.DEV) {
    return null;
  }

  const handleSelect = async (value: string) => {
    const level = value as LogLevel;
    if (level === currentLevel || isUpdating) {
      return;
    }

    setIsUpdating(true);
    try {
      await invoke("set_dev_console_log_level", { level });
      setCurrentLevel(level);
    } catch (error) {
      console.error("Failed to set Dev Console log level:", error);
    } finally {
      setIsUpdating(false);
    }
  };

  return (
    <SettingContainer
      title={
        <div>
          <div>{t("settings.debug.devConsoleLogLevel.title")}</div>
          <div className="mt-0.5 text-xs font-medium text-red-400">
            {t("settings.debug.devConsoleLogLevel.debugOnly")}
          </div>
        </div>
      }
      description={t("settings.debug.devConsoleLogLevel.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
      layout="horizontal"
    >
      <Dropdown
        options={DEV_CONSOLE_LOG_LEVEL_OPTIONS}
        selectedValue={currentLevel}
        onSelect={handleSelect}
        disabled={isLoading || isUpdating}
      />
    </SettingContainer>
  );
};
