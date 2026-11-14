import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { useTranslation } from "react-i18next";
import { SettingContainer } from "../ui/SettingContainer";
import { Dropdown } from "../ui/Dropdown";
import { Button } from "../ui/Button";

type StorageMode = "portable" | "desktop";

interface PortableStorageStatus {
  active_mode: StorageMode;
  configured_mode: StorageMode;
  portable_path: string;
  desktop_path: string;
}

export const PortableStorageSettings: React.FC = () => {
  const { t } = useTranslation();
  const [status, setStatus] = useState<PortableStorageStatus | null>(null);
  const [changing, setChanging] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let disposed = false;
    void invoke<PortableStorageStatus | null>("get_portable_storage")
      .then((value) => { if (!disposed) setStatus(value); })
      .catch((reason) => { if (!disposed) setError(String(reason)); });
    return () => { disposed = true; };
  }, []);

  const changeMode = async (mode: string) => {
    if (!status || mode === status.configured_mode) return;
    if (mode !== "portable" && mode !== "desktop") return;
    setChanging(true);
    setError(null);
    try {
      await invoke("set_portable_storage", { mode });
      setStatus({ ...status, configured_mode: mode });
    } catch (reason) {
      setError(String(reason));
    } finally {
      setChanging(false);
    }
  };

  const restart = async () => {
    setChanging(true);
    setError(null);
    try {
      await relaunch();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setChanging(false);
    }
  };

  if (!status && !error) return null;

  const label = (mode: StorageMode) => t(`settings.advanced.portableStorage.${mode}`);

  return (
    <SettingContainer
      id="settings-portable-storage"
      title={t("settings.advanced.portableStorage.title")}
      description={t("settings.advanced.portableStorage.description")}
      descriptionMode="inline"
      grouped={true}
      layout="stacked"
    >
      <div className="space-y-3">
        {status && (
          <>
            <Dropdown
              ariaLabel={t("settings.advanced.portableStorage.title")}
              selectedValue={status.configured_mode}
              options={(["portable", "desktop"] as const).map((mode) => ({
                value: mode,
                label: label(mode),
              }))}
              disabled={changing}
              onSelect={(mode) => void changeMode(mode)}
            />
            <p className="text-xs text-mid-gray">
              {t("settings.advanced.portableStorage.active", { location: label(status.active_mode) })}
            </p>
            <div className="text-xs font-mono break-all text-mid-gray">
              {status.configured_mode === "portable" ? status.portable_path : status.desktop_path}
            </div>
            <p className="text-xs text-mid-gray">
              {t("settings.advanced.portableStorage.keepData")}
            </p>
            {status.active_mode !== status.configured_mode && (
              <div className="space-y-2">
                <p className="text-sm text-amber-400">
                  {t("settings.advanced.portableStorage.restartRequired")}
                </p>
                <Button variant="secondary" size="sm" disabled={changing} onClick={() => void restart()}>
                  {t("settings.advanced.portableStorage.restart")}
                </Button>
              </div>
            )}
          </>
        )}
        {error && <p role="alert" className="text-sm text-red-400">{error}</p>}
      </div>
    </SettingContainer>
  );
};
