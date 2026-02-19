import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { AlertTriangle, Info, RefreshCw, CheckCircle } from "lucide-react";
import { SettingContainer } from "../../ui/SettingContainer";
import { TellMeMore } from "../../ui/TellMeMore";
import { useSettings } from "../../../hooks/useSettings";

// ShortcutBinding type from backend
interface ShortcutBinding {
  id: string;
  name: string;
  description: string;
  default_binding: string;
  current_binding: string;
}

export const ShortcutEngineSelector: React.FC = () => {
  const { t } = useTranslation();
  const { settings, isUpdating, refreshSettings } = useSettings();
  const [incompatibleShortcuts, setIncompatibleShortcuts] = useState<ShortcutBinding[]>([]);
  const [isChanging, setIsChanging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [activeEngine, setActiveEngine] = useState<string>("tauri");
  const [showRestartConfirm, setShowRestartConfirm] = useState(false);
  const [pendingEngine, setPendingEngine] = useState<string | null>(null);

  // Configured engine from settings (may differ from active if restart needed)
  const configuredEngine = (settings as any)?.shortcut_engine ?? "tauri";

  // Check if restart is required
  const needsRestart = activeEngine !== configuredEngine;

  // Fetch the currently active engine on mount
  useEffect(() => {
    fetchActiveEngine();
  }, []);

  // Fetch incompatible shortcuts when engine is tauri or when component mounts
  useEffect(() => {
    if (configuredEngine === "tauri") {
      fetchIncompatibleShortcuts();
    } else {
      setIncompatibleShortcuts([]);
    }
  }, [configuredEngine]);

  const fetchActiveEngine = async () => {
    try {
      const result = await invoke<string>("get_current_shortcut_engine");
      setActiveEngine(result);
    } catch (err) {
      console.error("Failed to fetch active engine:", err);
    }
  };

  const getIncompatibleShortcuts = async (): Promise<ShortcutBinding[]> => {
    try {
      return await invoke<ShortcutBinding[]>("get_tauri_incompatible_shortcuts");
    } catch (err) {
      console.error("Failed to fetch incompatible shortcuts:", err);
      return [];
    }
  };

  const fetchIncompatibleShortcuts = async () => {
    const shortcuts = await getIncompatibleShortcuts();
    setIncompatibleShortcuts(shortcuts);
  };

  const handleEngineChange = async (newEngine: string) => {
    if (newEngine === configuredEngine) return;

    setIsChanging(true);
    setError(null);

    try {
      if (newEngine === "tauri") {
        const shortcuts = await getIncompatibleShortcuts();
        setIncompatibleShortcuts(shortcuts);

        // Do not persist destructive changes until user explicitly confirms restart.
        if (shortcuts.length > 0) {
          setPendingEngine(newEngine);
          setShowRestartConfirm(true);
          return;
        }
      } else {
        setIncompatibleShortcuts([]);
      }

      setPendingEngine(null);
      await invoke("set_shortcut_engine_setting", { engine: newEngine });
      await refreshSettings();
    } catch (err: any) {
      console.error("Failed to change shortcut engine:", err);
      setError(err.toString());
    } finally {
      setIsChanging(false);
    }
  };

  const handleRestartClick = () => {
    // Show confirmation dialog if switching to tauri and there are incompatible shortcuts
    if (configuredEngine === "tauri" && incompatibleShortcuts.length > 0) {
      setShowRestartConfirm(true);
    } else {
      handleRestart();
    }
  };

  const handleRestart = async () => {
    setShowRestartConfirm(false);
    setIsChanging(true);
    try {
      if (pendingEngine) {
        await invoke("set_shortcut_engine_setting", { engine: pendingEngine });
        await refreshSettings();
        setPendingEngine(null);
      }
      await relaunch();
    } catch (err) {
      console.error("Failed to restart app:", err);
      setError(`Failed to restart: ${err}`);
    } finally {
      setIsChanging(false);
    }
  };

  const isDisabled = isChanging || isUpdating("shortcut_engine");

  return (
    <div className="space-y-3">
      {/* Visual separator from preceding items */}
      <div className="h-px bg-white/[0.05] mx-6 mt-2 mb-4" />
      <SettingContainer
        title={t("settings.debug.shortcutEngine.title")}
        description={
          <span className="text-gray-400">
            Choose the keyboard shortcut detection method. rdev supports all keys but uses more CPU:{" "}
            <span className="text-red-400 font-medium">read warning in "tell me more" below</span>.{" "}
            Tauri is faster but has limited key support.
          </span>
        }
        descriptionMode="inline"
        grouped={true}
      >
        <div className="flex items-center gap-3">
          <select
            value={configuredEngine}
            onChange={(e) => handleEngineChange(e.target.value)}
            disabled={isDisabled}
            className="bg-[#2b2b2b] border border-[#3c3c3c] rounded-lg px-3 py-2 text-sm min-w-[160px] focus:outline-none focus:ring-2 focus:ring-[#9b5de5]/40 disabled:opacity-50"
          >
            <option value="rdev">{t("settings.debug.shortcutEngine.options.rdev")}</option>
            <option value="tauri">{t("settings.debug.shortcutEngine.options.tauri")}</option>
          </select>
        </div>
      </SettingContainer>

      {/* Currently active engine indicator */}
      <div className="mx-4 p-3 bg-[#2b2b2b]/50 border border-[#3c3c3c] rounded-lg">
        <div className="flex items-center gap-2 text-xs text-gray-400">
          <CheckCircle className="w-4 h-4 text-green-400" />
          <span>
            {t("settings.debug.shortcutEngine.activeEngine")}: {" "}
            <span className="font-semibold text-gray-200">
              {activeEngine === "rdev" 
                ? t("settings.debug.shortcutEngine.options.rdev")
                : t("settings.debug.shortcutEngine.options.tauri")
              }
            </span>
          </span>
        </div>
      </div>

      {/* Restart required banner */}
      {needsRestart && (
        <div className="mx-4 p-3 bg-orange-500/10 border border-orange-500/30 rounded-lg">
          <div className="flex items-center justify-between gap-2">
            <div className="flex items-start gap-2">
              <RefreshCw className="w-4 h-4 text-orange-400 mt-0.5 flex-shrink-0" />
              <div className="text-xs text-orange-200/80">
                <p className="font-semibold">{t("settings.debug.shortcutEngine.restartRequired.title")}</p>
                <p>{t("settings.debug.shortcutEngine.restartRequired.description")}</p>
              </div>
            </div>
            <button
              onClick={handleRestartClick}
              className="px-3 py-1.5 bg-orange-500/20 hover:bg-orange-500/30 border border-orange-500/50 rounded-lg text-xs text-orange-200 font-medium transition-colors flex items-center gap-1.5"
            >
              <RefreshCw className="w-3 h-3" />
              {t("settings.debug.shortcutEngine.restartRequired.button")}
            </button>
          </div>
        </div>
      )}

      {/* Restart confirmation dialog with incompatible shortcuts warning */}
      {showRestartConfirm && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
          <div className="bg-[#1e1e1e] border border-[#3c3c3c] rounded-xl p-5 max-w-md mx-4 shadow-2xl">
            <div className="flex items-start gap-3 mb-4">
              <AlertTriangle className="w-6 h-6 text-yellow-400 flex-shrink-0 mt-0.5" />
              <div>
                <h3 className="text-base font-semibold text-white mb-1">
                  {t("settings.debug.shortcutEngine.confirmRestart.title")}
                </h3>
                <p className="text-sm text-gray-400">
                  {t("settings.debug.shortcutEngine.confirmRestart.description")}
                </p>
              </div>
            </div>

            {incompatibleShortcuts.length > 0 && (
              <div className="mb-4 p-3 bg-yellow-500/10 border border-yellow-500/30 rounded-lg">
                <p className="text-xs text-yellow-200/80 font-semibold mb-2">
                  {t("settings.debug.shortcutEngine.confirmRestart.incompatibleList")}
                </p>
                <ul className="text-xs text-yellow-200/70 space-y-1">
                  {incompatibleShortcuts.map((s) => (
                    <li key={s.id} className="flex items-center gap-2">
                      <span className="text-gray-400">•</span>
                      <span className="font-medium">{s.name}</span>
                      <code className="bg-black/30 px-1.5 py-0.5 rounded text-yellow-300">
                        {s.current_binding}
                      </code>
                    </li>
                  ))}
                </ul>
              </div>
            )}

            <div className="flex gap-3 justify-end">
              <button
                onClick={() => {
                  setShowRestartConfirm(false);
                  setPendingEngine(null);
                }}
                className="px-4 py-2 bg-[#2b2b2b] hover:bg-[#3c3c3c] border border-[#3c3c3c] rounded-lg text-sm text-gray-300 font-medium transition-colors"
              >
                {t("settings.debug.shortcutEngine.confirmRestart.cancel")}
              </button>
              <button
                onClick={handleRestart}
                className="px-4 py-2 bg-orange-500/20 hover:bg-orange-500/30 border border-orange-500/50 rounded-lg text-sm text-orange-200 font-medium transition-colors flex items-center gap-2"
              >
                <RefreshCw className="w-4 h-4" />
                {t("settings.debug.shortcutEngine.confirmRestart.confirm")}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Warning about incompatible shortcuts when switching to Tauri */}
      {configuredEngine === "tauri" && incompatibleShortcuts.length > 0 && (
        <div className="mx-4 p-3 bg-yellow-500/10 border border-yellow-500/30 rounded-lg">
          <div className="flex items-start gap-2">
            <AlertTriangle className="w-4 h-4 text-yellow-400 mt-0.5 flex-shrink-0" />
            <div className="text-xs text-yellow-200/80">
              <p className="font-semibold mb-1">{t("settings.debug.shortcutEngine.incompatibleWarning.title")}</p>
              <p className="mb-2">{t("settings.debug.shortcutEngine.incompatibleWarning.description")}</p>
              <ul className="list-disc list-inside space-y-1">
                {incompatibleShortcuts.map((s) => (
                  <li key={s.id}>
                    <span className="font-medium">{s.name}</span>
                    <code className="bg-black/30 px-1 ml-1 rounded">{s.current_binding}</code>
                  </li>
                ))}
              </ul>
            </div>
          </div>
        </div>
      )}

      {/* Detailed Help Info */}
      <div className="mx-4">
        <TellMeMore title={t("settings.debug.shortcutEngine.tellMeMore.title")}>
          <div className="space-y-6">
            <p className="text-gray-400 italic text-center text-xs">
              {t("settings.debug.shortcutEngine.tellMeMore.intro")}
            </p>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              {/* Native Engine Column */}
              <div className="space-y-4">
                <div className="flex items-center gap-2 pb-2 border-b border-white/5">
                  <CheckCircle className="w-5 h-5 text-accent" />
                  <h4 className="font-bold text-base text-accent">
                    {t("settings.debug.shortcutEngine.tellMeMore.tauri.title")}
                  </h4>
                </div>
                
                <p className="text-sm text-gray-300" dangerouslySetInnerHTML={{ __html: t("settings.debug.shortcutEngine.tellMeMore.tauri.description") }} />

                <div className="space-y-3">
                  <div className="bg-green-500/5 border border-green-500/10 p-3 rounded-lg shadow-sm">
                    <p className="text-xs font-bold text-green-400 uppercase tracking-wider mb-2">{t("settings.debug.shortcutEngine.tellMeMore.tauri.pros.title")}</p>
                    <ul className="text-xs text-gray-400 space-y-1.5 list-disc list-inside">
                      <li>{t("settings.debug.shortcutEngine.tellMeMore.tauri.pros.minimalOverhead")}</li>
                      <li>{t("settings.debug.shortcutEngine.tellMeMore.tauri.pros.security")}</li>
                    </ul>
                  </div>
                  <div className="bg-orange-500/5 border border-orange-500/10 p-3 rounded-lg shadow-sm font-light">
                    <p className="text-xs font-bold text-orange-400 uppercase tracking-wider mb-2">{t("settings.debug.shortcutEngine.tellMeMore.tauri.cons.title")}</p>
                    <ul className="text-xs text-gray-400 space-y-1.5 list-disc list-inside">
                      <li>{t("settings.debug.shortcutEngine.tellMeMore.tauri.cons.modifierOnly")}</li>
                      <li>{t("settings.debug.shortcutEngine.tellMeMore.tauri.cons.lockKeys")}</li>
                      <li>{t("settings.debug.shortcutEngine.tellMeMore.tauri.cons.conflicts")}</li>
                    </ul>
                  </div>
                </div>
              </div>

              {/* Hook Engine Column */}
              <div className="space-y-4">
                <div className="flex items-center gap-2 pb-2 border-b border-white/5">
                  <AlertTriangle className="w-5 h-5 text-yellow-500" />
                  <h4 className="font-bold text-base text-yellow-500">
                    {t("settings.debug.shortcutEngine.tellMeMore.rdev.title")}
                  </h4>
                </div>

                <p className="text-sm text-gray-300" dangerouslySetInnerHTML={{ __html: t("settings.debug.shortcutEngine.tellMeMore.rdev.description") }} />

                <div className="space-y-3">
                  <div className="bg-green-500/5 border border-green-500/10 p-3 rounded-lg shadow-sm">
                    <p className="text-xs font-bold text-green-400 uppercase tracking-wider mb-2">{t("settings.debug.shortcutEngine.tellMeMore.rdev.pros.title")}</p>
                    <ul className="text-xs text-gray-400 space-y-1.5 list-disc list-inside">
                      <li>{t("settings.debug.shortcutEngine.tellMeMore.rdev.pros.flexibility")}</li>
                      <li>{t("settings.debug.shortcutEngine.tellMeMore.rdev.pros.enhanced")}</li>
                    </ul>
                  </div>
                  <div className="bg-red-500/5 border border-red-500/20 p-3 rounded-lg shadow-sm font-light space-y-3">
                    <div>
                      <p className="text-xs font-bold text-red-400 uppercase tracking-wider mb-2">{t("settings.debug.shortcutEngine.tellMeMore.rdev.cons.title")}</p>
                      <ul className="text-xs text-gray-400 space-y-1.5 list-disc list-inside">
                        <li>{t("settings.debug.shortcutEngine.tellMeMore.rdev.cons.security")}</li>
                        <li>
                          {t("settings.debug.shortcutEngine.tellMeMore.rdev.cons.antiCheat")}
                          <div className="mt-1.5 mb-2 px-2 border-l border-red-500/30 ml-4">
                            <p className="text-[11px] leading-relaxed text-red-100/80 italic font-normal" dangerouslySetInnerHTML={{ __html: t("settings.debug.shortcutEngine.tellMeMore.rdev.cons.harshReality") }} />
                          </div>
                        </li>
                        <li>{t("settings.debug.shortcutEngine.tellMeMore.rdev.cons.performance")}</li>
                      </ul>
                    </div>
                  </div>
                </div>
              </div>
            </div>

            {/* Verdict */}
            <div className="p-4 bg-accent/5 border border-accent/20 rounded-xl flex items-start gap-3">
              <Info className="w-5 h-5 text-accent mt-0.5 flex-shrink-0" />
              <div>
                <h4 className="text-xs font-bold text-accent uppercase tracking-widest mb-1">
                  {t("settings.debug.shortcutEngine.tellMeMore.verdict.title")}
                </h4>
                <p className="text-sm text-gray-300">
                  {t("settings.debug.shortcutEngine.tellMeMore.verdict.text")}
                </p>
              </div>
            </div>
          </div>
        </TellMeMore>
      </div>

      {/* Error display */}
      {error && (
        <div className="mx-4 p-3 bg-red-500/10 border border-red-500/30 rounded-lg">
          <div className="flex items-start gap-2">
            <AlertTriangle className="w-4 h-4 text-red-400 mt-0.5 flex-shrink-0" />
            <div className="text-xs text-red-200/80">
              <p className="font-semibold mb-1">{t("settings.debug.shortcutEngine.error")}</p>
              <p>{error}</p>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
