import React, { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { type } from "@tauri-apps/plugin-os";
import {
  formatKeyCombination,
  isModifierOnlyShortcut,
  type OSType,
} from "../../lib/utils/keyboard";
import { useKeyboardCapture } from "@/hooks/useKeyboardCapture";
import { ResetButton } from "../ui/ResetButton";
import { SettingContainer } from "../ui/SettingContainer";
import { useSettings } from "../../hooks/useSettings";
import { sessionToast as toast } from "@/lib/sessionToast";
import { showShortcutSetErrorToast } from "../../lib/utils/shortcutEngineErrorToast";

interface GlobalShortcutInputProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  shortcutId: string;
  disabled?: boolean;
}

interface BindingSuspension {
  promise: Promise<boolean>;
  releaseRequested: boolean;
  resumeStarted: boolean;
}

export const GlobalShortcutInput: React.FC<GlobalShortcutInputProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
  shortcutId,
  disabled = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateBinding, resetBinding, isUpdating, isLoading } =
    useSettings();
  const [editingShortcutId, setEditingShortcutId] = useState<string | null>(
    null,
  );
  const [originalBinding, setOriginalBinding] = useState<string>("");
  const [osType, setOsType] = useState<OSType>("unknown");
  const suspensionRef = useRef<BindingSuspension | null>(null);

  const bindings = getSetting("bindings") || {};
  const configuredShortcutEngine =
    (getSetting("shortcut_engine") as string | undefined) ?? "handy_keys";

  useEffect(() => {
    const detectOsType = async () => {
      try {
        const detectedType = type();
        let normalizedType: OSType;

        switch (detectedType) {
          case "macos":
            normalizedType = "macos";
            break;
          case "windows":
            normalizedType = "windows";
            break;
          case "linux":
            normalizedType = "linux";
            break;
          default:
            normalizedType = "unknown";
        }

        setOsType(normalizedType);
      } catch (error) {
        console.error("Error detecting OS type:", error);
        setOsType("unknown");
      }
    };

    detectOsType();
  }, []);

  const releaseBindingSuspension = useCallback(async () => {
    const suspension = suspensionRef.current;
    if (!suspension) return;

    // Queue release behind an in-flight suspend command. This matters when the
    // component unmounts before suspend_all_bindings has returned.
    suspension.releaseRequested = true;
    const suspended = await suspension.promise;
    if (suspended && !suspension.resumeStarted) {
      suspension.resumeStarted = true;
      await invoke("resume_all_bindings").catch(console.error);
    }
    if (suspensionRef.current === suspension) {
      suspensionRef.current = null;
    }
  }, []);

  useEffect(() => {
    return () => {
      void releaseBindingSuspension();
    };
  }, [releaseBindingSuspension]);

  const finishRecording = useCallback(() => {
    setEditingShortcutId(null);
    setOriginalBinding("");
  }, []);

  const cancelRecording = useCallback(async () => {
    if (editingShortcutId && originalBinding) {
      try {
        await updateBinding(editingShortcutId, originalBinding);
      } catch (error) {
        toast.error(
          t("settings.general.shortcut.errors.restore", {
            error: String(error),
          }),
        );
      }
    }
    await releaseBindingSuspension();
    finishRecording();
  }, [
    editingShortcutId,
    finishRecording,
    originalBinding,
    releaseBindingSuspension,
    t,
    updateBinding,
  ]);

  const saveCapturedShortcut = useCallback(
    async (newShortcut: string) => {
      const activeShortcutId = editingShortcutId;
      if (!activeShortcutId || !bindings[activeShortcutId]) {
        await releaseBindingSuspension();
        finishRecording();
        return;
      }

      try {
        await updateBinding(activeShortcutId, newShortcut);

        if (
          osType === "windows" &&
          isModifierOnlyShortcut(newShortcut)
        ) {
          toast.warning(t("settings.general.shortcut.warnings.modifierOnly"), {
            duration: 6000,
          });
        }
      } catch (error) {
        console.error("Failed to change binding:", error);
        showShortcutSetErrorToast(error, configuredShortcutEngine, t);

        if (originalBinding) {
          try {
            await updateBinding(activeShortcutId, originalBinding);
          } catch (resetError) {
            toast.error(
              t("settings.general.shortcut.errors.reset", {
                error: String(resetError),
              }),
            );
          }
        }
      }

      await releaseBindingSuspension();
      finishRecording();
    },
    [
      bindings,
      configuredShortcutEngine,
      editingShortcutId,
      finishRecording,
      originalBinding,
      osType,
      releaseBindingSuspension,
      t,
      updateBinding,
    ],
  );

  const { containerRef, displayKeys } = useKeyboardCapture({
    isCapturing: editingShortcutId === shortcutId,
    osType,
    onCaptured: saveCapturedShortcut,
    onCancel: cancelRecording,
  });

  const startRecording = async (id: string) => {
    if (editingShortcutId !== null || suspensionRef.current) return;

    const suspension: BindingSuspension = {
      promise: Promise.resolve(false),
      releaseRequested: false,
      resumeStarted: false,
    };
    suspension.promise = invoke("suspend_all_bindings")
      .then(() => true)
      .catch((error) => {
        console.error(error);
        return false;
      });
    suspensionRef.current = suspension;

    const suspended = await suspension.promise;
    if (!suspended) {
      if (suspensionRef.current === suspension) {
        suspensionRef.current = null;
      }
      return;
    }
    if (suspension.releaseRequested) {
      await releaseBindingSuspension();
      return;
    }

    setOriginalBinding(bindings[id]?.current_binding || "");
    setEditingShortcutId(id);
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
        {editingShortcutId === shortcutId ? (
          <div
            ref={containerRef}
            role="status"
            aria-live="polite"
            className="px-2 py-1 text-sm font-semibold border border-logo-primary bg-logo-primary/30 rounded min-w-[120px] text-center"
          >
            {displayKeys || t("settings.general.shortcut.pressKeys")}
          </div>
        ) : (
          <button
            type="button"
            className="px-2 py-1 text-sm font-semibold bg-mid-gray/10 border border-mid-gray/80 hover:bg-logo-primary/10 rounded cursor-pointer hover:border-logo-primary min-w-[120px] text-center"
            onClick={() => startRecording(shortcutId)}
            disabled={disabled}
          >
            {binding.current_binding
              ? formatKeyCombination(binding.current_binding, osType)
              : t("settings.general.shortcut.notSet")}
          </button>
        )}
        <ResetButton
          onClick={() => resetBinding(shortcutId)}
          disabled={isUpdating(`binding_${shortcutId}`)}
        />
      </div>
    </SettingContainer>
  );
};
