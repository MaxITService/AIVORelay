import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands } from "@/bindings";
import { SettingContainer } from "../../ui/SettingContainer";
import { Button } from "../../ui/Button";
import { Check, Copy, FolderOpen } from "lucide-react";

interface SettingsDirectoryProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

export const SettingsDirectory: React.FC<SettingsDirectoryProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const [appDir, setAppDir] = useState<string>("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  // Load the app directory path
  useEffect(() => {
    const loadAppDirectory = async () => {
      try {
        const result = await commands.getAppDirPath();
        if (result.status === "ok") {
          setAppDir(result.data);
        } else {
          setError(result.error);
        }
      } catch (err) {
        const errorMessage =
          err && typeof err === "object" && "message" in err
            ? String(err.message)
            : "Failed to load directory";
        setError(errorMessage);
      } finally {
        setLoading(false);
      }
    };

    void loadAppDirectory();
  }, []);

  // Open the directory via Tauri command
  const handleOpen = async () => {
    if (!appDir) return;
    try {
      await commands.openAppDataDir();
    } catch (openError) {
      console.error("Failed to open directory:", openError);
    }
  };

  // Copy path to clipboard
  const handleCopy = async () => {
    if (!appDir) return;
    try {
      await navigator.clipboard.writeText(appDir);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy to clipboard:", err);
    }
  };

  return (
    <SettingContainer
      title={t("settings.debug.settingsDirectoryTitle", "Settings Directory")}
      description={t(
        "settings.debug.settingsDirectoryDescription",
        "Your settings are stored in this folder in the settings_store.json file. You can copy it to transfer your settings to a new computer."
      )}
      descriptionMode={descriptionMode}
      grouped={grouped}
      layout="stacked"
    >
      {loading ? (
        <div className="animate-pulse">
          <div className="h-8 bg-gray-100 rounded" />
        </div>
      ) : error ? (
        <div className="p-3 bg-red-50 border border-red-200 rounded text-xs text-red-600">
          {t("errors.loadDirectory", { error })}
        </div>
      ) : (
        <div className="flex items-center gap-2">
          <div className="flex-1 min-w-0 px-2 py-2 bg-mid-gray/10 border border-mid-gray/80 rounded text-xs font-mono break-all">
            {appDir}
          </div>
          <Button
            onClick={handleCopy}
            variant="secondary"
            size="sm"
            disabled={!appDir}
            className="px-3 py-2 flex items-center gap-1.5"
          >
            {copied ? (
              <>
                <Check className="w-4 h-4 text-green-500" />
                <span>{t("common.copied", "Copied")}</span>
              </>
            ) : (
              <>
                <Copy className="w-4 h-4" />
                <span>{t("common.copy", "Copy")}</span>
              </>
            )}
          </Button>
          <Button
            onClick={handleOpen}
            variant="secondary"
            size="sm"
            disabled={!appDir}
            className="px-3 py-2 flex items-center gap-1.5"
          >
            <FolderOpen className="w-4 h-4" />
            <span>{t("common.open", "Open")}</span>
          </Button>
        </div>
      )}
    </SettingContainer>
  );
};
