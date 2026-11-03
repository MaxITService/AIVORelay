import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../../hooks/useSettings";
import { SettingContainer } from "../../ui/SettingContainer";

type LinuxBackend = "tauri" | "none";

export const LinuxShortcutBackend: React.FC = () => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();
  const [active, setActive] = useState<LinuxBackend | null>(null);
  const [changing, setChanging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const configured = (settings as { shortcut_engine?: string } | null)?.shortcut_engine === "none"
    ? "none" : "tauri";
  const label = (backend: LinuxBackend) => backend === "none"
    ? t("settings.debug.linuxShortcutBackend.none") : "Tauri";
  useEffect(() => {
    let disposed = false;
    void invoke<LinuxBackend>("get_current_shortcut_engine").then((engine) => {
      if (!disposed) setActive(engine);
    }).catch((reason) => { if (!disposed) setError(String(reason)); });
    return () => { disposed = true; };
  }, []);

  const changeBackend = async (engine: string) => {
    if (engine === configured) return;
    setChanging(true);
    setError(null);
    try {
      await invoke("set_shortcut_engine_setting", { engine });
      await refreshSettings();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setChanging(false);
    }
  };
  const restart = async () => {
    setChanging(true);
    setError(null);
    try { await relaunch(); }
    catch (reason) { setError(String(reason)); }
    finally { setChanging(false); }
  };

  return (
    <div className="space-y-3 pb-4">
      <SettingContainer
        id="settings-linux-shortcut-backend"
        title={t("settings.debug.linuxShortcutBackend.title")}
        description={t("settings.debug.linuxShortcutBackend.description")}
        descriptionMode="inline"
        grouped={true}
      >
        <select
          aria-label={t("settings.debug.linuxShortcutBackend.title")}
          value={configured}
          disabled={changing || active === null || !settings}
          onChange={(event) => void changeBackend(event.target.value)}
          className="bg-background border border-mid-gray/30 rounded-lg px-3 py-2 text-sm disabled:opacity-50"
        >
          <option value="tauri">Tauri</option>
          <option value="none">{label("none")}</option>
        </select>
      </SettingContainer>
      <div className="px-6 space-y-2 text-sm text-text/70">
        {active && <p>{t("settings.debug.linuxShortcutBackend.active", { backend: label(active) })}</p>}
        {(configured === "none" || active === "none") && (
          <p>{t("settings.debug.linuxShortcutBackend.commands")}</p>
        )}
        {active && active !== configured && (
          <div className="space-y-2">
            <p className="text-amber-400">{t("settings.debug.linuxShortcutBackend.restartRequired")}</p>
            <button type="button" onClick={() => void restart()} disabled={changing}
              className="px-3 py-1.5 rounded-lg border border-mid-gray/30 hover:bg-mid-gray/10 disabled:opacity-50">
              {t("settings.debug.linuxShortcutBackend.restart")}
            </button>
          </div>
        )}
        {error && <p role="alert" className="text-red-400">{error}</p>}
      </div>
    </div>
  );
};
