import React, { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { type } from "@tauri-apps/plugin-os";
import { useTranslation } from "react-i18next";
import { formatKeyCombination, type OSType } from "../../lib/utils/keyboard";
import { useSettings } from "../../hooks/useSettings";
import { ResetButton } from "../ui/ResetButton";
import { SettingContainer } from "../ui/SettingContainer";
import { toast } from "sonner";
import { showShortcutSetErrorToast } from "../../lib/utils/shortcutEngineErrorToast";

interface HandyKeysShortcutInputProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  shortcutId: string;
  disabled?: boolean;
}

interface HandyKeysEvent {
  modifiers: string[];
  key: string | null;
  is_key_down: boolean;
  hotkey_string: string;
}

export const HandyKeysShortcutInput: React.FC<HandyKeysShortcutInputProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
  shortcutId,
  disabled = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateBinding, resetBinding, isUpdating, isLoading } =
    useSettings();
  const [isRecording, setIsRecording] = useState(false);
  const [currentKeys, setCurrentKeys] = useState("");
  const [originalBinding, setOriginalBinding] = useState("");
  const [osType, setOsType] = useState<OSType>("unknown");
  const shortcutRef = useRef<HTMLDivElement | null>(null);
  const unlistenRef = useRef<(() => void) | null>(null);
  const currentKeysRef = useRef("");

  const bindings = getSetting("bindings") || {};
  const configuredShortcutEngine =
    (getSetting("shortcut_engine") as string | undefined) ?? "handy_keys";

  useEffect(() => {
    try {
      const detectedType = type();
      setOsType(
        detectedType === "macos" ||
          detectedType === "windows" ||
          detectedType === "linux"
          ? detectedType
          : "unknown",
      );
    } catch (error) {
      console.error("Error detecting OS type:", error);
      setOsType("unknown");
    }
  }, []);

  const stopRecordingSession = useCallback(async () => {
    if (unlistenRef.current) {
      unlistenRef.current();
      unlistenRef.current = null;
    }

    await invoke("stop_handy_keys_recording").catch(console.error);
    setIsRecording(false);
    setCurrentKeys("");
    currentKeysRef.current = "";
  }, []);

  const cancelRecording = useCallback(async () => {
    if (!isRecording) return;

    await stopRecordingSession();

    if (originalBinding) {
      try {
        await updateBinding(shortcutId, originalBinding);
      } catch (error) {
        toast.error(
          t("settings.general.shortcut.errors.restore", {
            error: String(error),
          }),
        );
      }
    }

    setOriginalBinding("");
  }, [
    isRecording,
    originalBinding,
    shortcutId,
    stopRecordingSession,
    t,
    updateBinding,
  ]);

  useEffect(() => {
    if (!isRecording) return;

    let disposed = false;

    const setupListener = async () => {
      const unlisten = await listen<HandyKeysEvent>(
        "handy-keys-event",
        async (event) => {
          if (disposed) return;

          const { hotkey_string, is_key_down } = event.payload;

          if (is_key_down && hotkey_string) {
            currentKeysRef.current = hotkey_string;
            setCurrentKeys(hotkey_string);
            return;
          }

          if (!is_key_down && currentKeysRef.current) {
            const nextShortcut = currentKeysRef.current;

            try {
              await updateBinding(shortcutId, nextShortcut);
            } catch (error) {
              showShortcutSetErrorToast(error, configuredShortcutEngine, t);

              if (originalBinding) {
                try {
                  await updateBinding(shortcutId, originalBinding);
                } catch (resetError) {
                  toast.error(
                    t("settings.general.shortcut.errors.reset", {
                      error: String(resetError),
                    }),
                  );
                }
              }
            }

            await stopRecordingSession();
            setOriginalBinding("");
          }
        },
      );

      unlistenRef.current = unlisten;
    };

    setupListener().catch((error) => {
      console.error("Failed to listen for handy-keys events:", error);
    });

    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        cancelRecording().catch(console.error);
      }
    };

    window.addEventListener("keydown", handleEscape);

    return () => {
      disposed = true;
      window.removeEventListener("keydown", handleEscape);
      stopRecordingSession().catch(console.error);
    };
  }, [
    cancelRecording,
    isRecording,
    originalBinding,
    shortcutId,
    stopRecordingSession,
    t,
    updateBinding,
  ]);

  useEffect(() => {
    if (!isRecording) return;

    const handleClickOutside = (event: MouseEvent) => {
      if (
        shortcutRef.current &&
        !shortcutRef.current.contains(event.target as Node)
      ) {
        cancelRecording().catch(console.error);
      }
    };

    window.addEventListener("click", handleClickOutside);
    return () => window.removeEventListener("click", handleClickOutside);
  }, [cancelRecording, isRecording]);

  const startRecording = async () => {
    if (isRecording) return;

    try {
      await invoke("suspend_binding", { id: shortcutId });
      setOriginalBinding(bindings[shortcutId]?.current_binding || "");
      await invoke("start_handy_keys_recording", { bindingId: shortcutId });
      setIsRecording(true);
      setCurrentKeys("");
      currentKeysRef.current = "";
    } catch (error) {
      await invoke("resume_binding", { id: shortcutId }).catch(console.error);
      setOriginalBinding("");
      showShortcutSetErrorToast(error, configuredShortcutEngine, t);
    }
  };

  const formatCurrentKeys = () => {
    if (!currentKeys) {
      return t("settings.general.shortcut.pressKeys");
    }
    return formatKeyCombination(currentKeys, osType);
  };

  if (isLoading) {
    return (
      <SettingContainer
        title={t("settings.general.shortcut.title")}
        description={t("settings.general.shortcut.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <div className="text-sm text-mid-gray">
          {t("settings.general.shortcut.loading")}
        </div>
      </SettingContainer>
    );
  }

  if (Object.keys(bindings).length === 0) {
    return (
      <SettingContainer
        title={t("settings.general.shortcut.title")}
        description={t("settings.general.shortcut.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <div className="text-sm text-mid-gray">
          {t("settings.general.shortcut.none")}
        </div>
      </SettingContainer>
    );
  }

  const binding = bindings[shortcutId];
  if (!binding) {
    return (
      <SettingContainer
        title={t("settings.general.shortcut.title")}
        description={t("settings.general.shortcut.notFound")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <div className="text-sm text-mid-gray">
          {t("settings.general.shortcut.none")}
        </div>
      </SettingContainer>
    );
  }

  const translatedName = t(
    `settings.general.shortcut.bindings.${shortcutId}.name`,
    binding.name,
  );
  const translatedDescription = t(
    `settings.general.shortcut.bindings.${shortcutId}.description`,
    binding.description,
  );

  return (
    <SettingContainer
      title={translatedName}
      description={translatedDescription}
      descriptionMode={descriptionMode}
      grouped={grouped}
      disabled={disabled}
      layout="horizontal"
    >
      <div className="flex items-center space-x-1">
        {isRecording ? (
          <div
            ref={shortcutRef}
            className="px-2 py-1 text-sm font-semibold border border-logo-primary bg-logo-primary/30 rounded min-w-[120px] text-center"
          >
            {formatCurrentKeys()}
          </div>
        ) : (
          <div
            className="px-2 py-1 text-sm font-semibold bg-mid-gray/10 border border-mid-gray/80 hover:bg-logo-primary/10 rounded cursor-pointer hover:border-logo-primary min-w-[120px] text-center"
            onClick={startRecording}
          >
            {binding.current_binding
              ? formatKeyCombination(binding.current_binding, osType)
              : t("settings.general.shortcut.notSet")}
          </div>
        )}
        <ResetButton
          onClick={() => resetBinding(shortcutId)}
          disabled={isUpdating(`binding_${shortcutId}`)}
        />
      </div>
    </SettingContainer>
  );
};
