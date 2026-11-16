import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { BookOpen, ChevronDown } from "lucide-react";
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
  const [loadAttempt, setLoadAttempt] = useState(0);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    let disposed = false;
    setLoading(true);
    void invoke<PortableStorageStatus | null>("get_portable_storage")
      .then((value) => {
        if (!disposed) {
          setStatus(value);
          setError(null);
        }
      })
      .catch((reason) => { if (!disposed) setError(String(reason)); })
      .finally(() => { if (!disposed) setLoading(false); });
    return () => { disposed = true; };
  }, [loadAttempt]);

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
  const activePath = status
    ? status.active_mode === "portable"
      ? status.portable_path
      : status.desktop_path
    : "";
  const configuredPath = status
    ? status.configured_mode === "portable"
      ? status.portable_path
      : status.desktop_path
    : "";
  const pendingRestart = status ? status.active_mode !== status.configured_mode : false;

  const helpSections = [
    {
      key: "session",
      title: t("settings.advanced.portableStorage.help.sessionTitle"),
      body: t("settings.advanced.portableStorage.help.sessionBody"),
    },
    {
      key: "data",
      title: t("settings.advanced.portableStorage.help.dataTitle"),
      body: t("settings.advanced.portableStorage.help.dataBody"),
    },
    {
      key: "writable",
      title: t("settings.advanced.portableStorage.help.writableTitle"),
      body: t("settings.advanced.portableStorage.help.writableBody"),
    },
    {
      key: "moving",
      title: t("settings.advanced.portableStorage.help.movingTitle"),
      body: t("settings.advanced.portableStorage.help.movingBody"),
    },
    {
      key: "updates",
      title: t("settings.advanced.portableStorage.help.updatesTitle"),
      body: t("settings.advanced.portableStorage.help.updatesBody"),
    },
    {
      key: "caveats",
      title: t("settings.advanced.portableStorage.help.caveatsTitle"),
      body: t("settings.advanced.portableStorage.help.caveatsBody"),
    },
  ];

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
            <div className="space-y-2 rounded-lg border border-mid-gray/20 bg-mid-gray/5 px-3 py-2.5 text-xs">
              <div>
                <p className="font-medium text-text/85">
                  {t("settings.advanced.portableStorage.active", {
                    location: label(status.active_mode),
                  })}
                </p>
                <div className="mt-0.5 font-mono break-all text-mid-gray">
                  {activePath}
                </div>
              </div>
              {pendingRestart && (
                <div className="border-t border-mid-gray/15 pt-2">
                  <p className="font-medium text-amber-300">
                    {t("settings.advanced.portableStorage.afterRestart", {
                      location: label(status.configured_mode),
                    })}
                  </p>
                  <div className="mt-0.5 font-mono break-all text-amber-200/80">
                    {configuredPath}
                  </div>
                </div>
              )}
            </div>
            <p className="text-xs text-mid-gray">
              {t("settings.advanced.portableStorage.keepData")}
            </p>
            {pendingRestart && (
              <div className="space-y-2">
                <p className="text-sm text-amber-400">
                  {t("settings.advanced.portableStorage.restartRequired")}
                </p>
                <Button variant="secondary" size="sm" disabled={changing} onClick={() => void restart()}>
                  {t("settings.advanced.portableStorage.restart")}
                </Button>
              </div>
            )}
            <details className="group rounded-lg border border-mid-gray/20 bg-mid-gray/5 overflow-hidden">
              <summary className="flex cursor-pointer list-none items-center justify-between gap-3 rounded-lg px-3 py-2.5 text-xs font-medium text-[#d7b9ff] outline-none transition-colors hover:bg-mid-gray/10 hover:text-[#eadcff] focus-visible:ring-2 focus-visible:ring-[#ff4d8d]/40 [&::-webkit-details-marker]:hidden">
                <span className="flex min-w-0 items-center gap-2">
                  <BookOpen className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
                  <span>{t("settings.advanced.portableStorage.help.summary")}</span>
                </span>
                <ChevronDown
                  className="h-4 w-4 shrink-0 transition-transform group-open:rotate-180"
                  aria-hidden="true"
                />
              </summary>

              <div className="space-y-3 border-t border-mid-gray/20 px-3 py-3 text-xs leading-relaxed text-text/75">
                <div className="space-y-2">
                  <h4 className="font-semibold text-text/90">
                    {t("settings.advanced.portableStorage.help.modesTitle")}
                  </h4>
                  <div className="space-y-2 rounded-md border border-mid-gray/15 bg-black/15 p-2.5">
                    <div>
                      <p className="font-medium text-text/85">{label("portable")}</p>
                      <p className="mt-0.5 text-text/70">
                        {t("settings.advanced.portableStorage.help.portableMode")}
                      </p>
                      <div className="mt-1 font-mono break-all text-mid-gray">
                        {status.portable_path}
                      </div>
                    </div>
                    <div className="border-t border-mid-gray/15 pt-2">
                      <p className="font-medium text-text/85">{label("desktop")}</p>
                      <p className="mt-0.5 text-text/70">
                        {t("settings.advanced.portableStorage.help.desktopMode")}
                      </p>
                      <div className="mt-1 font-mono break-all text-mid-gray">
                        {status.desktop_path}
                      </div>
                    </div>
                  </div>
                </div>

                <dl className="space-y-2.5">
                  {helpSections.map((section) => (
                    <div key={section.key} className="space-y-0.5">
                      <dt className="font-semibold text-text/90">{section.title}</dt>
                      <dd className="text-text/70">{section.body}</dd>
                    </div>
                  ))}
                </dl>
              </div>
            </details>
          </>
        )}
        {error && <p role="alert" className="text-sm text-red-400">{error}</p>}
        {!status && error && (
          <Button
            variant="secondary"
            size="sm"
            disabled={loading || changing}
            onClick={() => {
              setLoading(true);
              setLoadAttempt((attempt) => attempt + 1);
            }}
          >
            {t("common.retry")}
          </Button>
        )}
      </div>
    </SettingContainer>
  );
};
