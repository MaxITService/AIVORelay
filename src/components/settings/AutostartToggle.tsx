import React from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { invoke } from "@tauri-apps/api/core";
import { type as getOsType } from "@tauri-apps/plugin-os";
import { sessionToast as toast } from "@/lib/sessionToast";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { Button } from "../ui/Button";
import { useSettings } from "../../hooks/useSettings";

interface AutostartToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

type AdministratorAccountState =
  | "checking"
  | "administrator"
  | "standard"
  | "error";

type AdminTaskHealth = "ok" | "missing" | "needsRepair" | "unknown";

// Backend error codes from shortcut.rs; rollback errors may wrap them.
const toAdminAutostartErrorMessage = (
  error: unknown,
  t: TFunction,
): string => {
  const message = String(error);
  if (message.includes("ADMIN_AUTOSTART_UAC_DECLINED")) {
    return t(
      "settings.advanced.autostartAsAdmin.uacDeclined",
      "Windows did not grant administrator permission (the UAC prompt was declined or closed), so the setting was not changed.",
    );
  }
  if (message.includes("ADMIN_AUTOSTART_TASK_FAILED")) {
    return t(
      "settings.advanced.autostartAsAdmin.taskFailed",
      "Windows could not register the administrator autostart task, so the setting was not changed.",
    );
  }
  return message;
};

export const AutostartToggle: React.FC<AutostartToggleProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating, refreshSettings } =
      useSettings();

    const autostartEnabled = getSetting("autostart_enabled") ?? false;
    const autostartAsAdmin =
      getSetting("autostart_as_admin_enabled") ?? false;
    const hideAdminInWindowTitle =
      getSetting("hide_admin_in_window_title") ?? false;
    const isWindows = getOsType() === "windows";
    const [administratorAccountState, setAdministratorAccountState] =
      React.useState<AdministratorAccountState>(
        isWindows ? "checking" : "standard",
      );

    React.useEffect(() => {
      if (!isWindows) {
        setAdministratorAccountState("standard");
        return;
      }

      let cancelled = false;
      setAdministratorAccountState("checking");
      void invoke<boolean>("is_current_user_administrator_account")
        .then((isAdministrator) => {
          if (!cancelled) {
            setAdministratorAccountState(
              isAdministrator ? "administrator" : "standard",
            );
          }
        })
        .catch((error) => {
          console.error(
            "Failed to determine Windows administrator account membership:",
            error,
          );
          if (!cancelled) {
            setAdministratorAccountState("error");
          }
        });

      return () => {
        cancelled = true;
      };
    }, [isWindows]);

    const handleAutostartChange = async (enabled: boolean) => {
      try {
        await updateSetting("autostart_enabled", enabled, {
          throwOnError: true,
        });
      } catch (error) {
        toast.error(String(error));
      } finally {
        // Disabling regular autostart also disables the administrator mode.
        await refreshSettings();
      }
    };

    const handleAutostartAsAdminChange = async (enabled: boolean) => {
      if (enabled && administratorAccountState !== "administrator") {
        toast.error(
          t(
            "settings.advanced.autostartAsAdmin.requiresAdministratorAccount",
            "This option is available only when the current Windows account belongs to the Administrators group.",
          ),
        );
        return;
      }

      try {
        await updateSetting("autostart_as_admin_enabled", enabled, {
          throwOnError: true,
        });
      } catch (error) {
        toast.error(toAdminAutostartErrorMessage(error, t));
      } finally {
        await refreshSettings();
      }
    };

    // Detect a broken or outdated administrator task (older versions saved
    // a broken path and Windows defaults); repair only on the user's click.
    const [adminTaskHealth, setAdminTaskHealth] =
      React.useState<AdminTaskHealth>("ok");
    const [isRepairingAdminTask, setIsRepairingAdminTask] =
      React.useState(false);

    React.useEffect(() => {
      if (!isWindows || !autostartAsAdmin) {
        setAdminTaskHealth("ok");
        return;
      }

      let cancelled = false;
      void invoke<AdminTaskHealth>("get_admin_autostart_task_health")
        .then((health) => {
          if (!cancelled) setAdminTaskHealth(health);
        })
        .catch((error) => {
          console.error("Failed to check administrator autostart task:", error);
        });

      return () => {
        cancelled = true;
      };
    }, [isWindows, autostartAsAdmin]);

    const handleRepairAdminTask = async () => {
      setIsRepairingAdminTask(true);
      try {
        await invoke("repair_admin_autostart_task");
        // The warning disappears once the re-check reports "ok".
        setAdminTaskHealth(
          await invoke<AdminTaskHealth>("get_admin_autostart_task_health"),
        );
      } catch (error) {
        toast.error(toAdminAutostartErrorMessage(error, t));
      } finally {
        setIsRepairingAdminTask(false);
      }
    };

    const autostartAsAdminDescription =
      administratorAccountState === "standard"
        ? t(
            "settings.advanced.autostartAsAdmin.requiresAdministratorAccount",
            "This option is available only when the current Windows account belongs to the Administrators group.",
          )
        : administratorAccountState === "error"
          ? t(
              "settings.advanced.autostartAsAdmin.administratorCheckFailed",
              "Windows administrator account membership could not be verified, so this option is unavailable.",
            )
          : t(
              "settings.advanced.autostartAsAdmin.description",
              "Launch AivoRelay with elevated permissions when you sign in to Windows, enabling dictation into administrator windows.",
            );

    return (
      <>
        <ToggleSwitch
          id="settings-autostart"
          checked={autostartEnabled}
          onChange={(enabled) => void handleAutostartChange(enabled)}
          isUpdating={isUpdating("autostart_enabled")}
          label={t("settings.advanced.autostart.label")}
          description={t("settings.advanced.autostart.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        />
        {isWindows && autostartEnabled && (
          <>
            <ToggleSwitch
              checked={autostartAsAdmin}
              onChange={(enabled) => void handleAutostartAsAdminChange(enabled)}
              disabled={
                administratorAccountState !== "administrator" &&
                !autostartAsAdmin
              }
              isUpdating={isUpdating("autostart_as_admin_enabled")}
              label={t(
                "settings.advanced.autostartAsAdmin.label",
                "Autostart with Administrator Privileges",
              )}
              description={autostartAsAdminDescription}
              descriptionMode={descriptionMode}
              grouped={grouped}
            />
            <p className="mx-4 mb-2 text-xs font-semibold text-amber-300">
              {t(
                "settings.advanced.autostartAsAdmin.notRecommended",
                "Not recommended, for IT professionals only: any bug in the application will have a huge blast radius.",
              )}
            </p>
            {autostartAsAdmin &&
              (adminTaskHealth === "missing" ||
                adminTaskHealth === "needsRepair") && (
                <div className="mx-4 mb-2 p-2.5 bg-amber-500/10 border border-amber-500/25 rounded-lg text-xs text-amber-200/90 leading-relaxed flex items-center gap-3">
                  <span className="flex-1">
                    {adminTaskHealth === "missing"
                      ? t(
                          "settings.advanced.autostartAsAdmin.repair.missing",
                          "The Windows task that starts AivoRelay as administrator is missing, so the app will not start when you sign in.",
                        )
                      : t(
                          "settings.advanced.autostartAsAdmin.repair.needsRepair",
                          "The Windows task that starts AivoRelay as administrator is outdated or broken, so the app may not start when you sign in.",
                        )}
                  </span>
                  <Button
                    variant="secondary"
                    size="sm"
                    disabled={isRepairingAdminTask}
                    onClick={() => void handleRepairAdminTask()}
                  >
                    {t(
                      "settings.advanced.autostartAsAdmin.repair.button",
                      "Repair",
                    )}
                  </Button>
                </div>
              )}
            {autostartAsAdmin && (
              <ToggleSwitch
                checked={hideAdminInWindowTitle}
                onChange={(enabled) =>
                  void updateSetting("hide_admin_in_window_title", enabled)
                }
                isUpdating={isUpdating("hide_admin_in_window_title")}
                label={t(
                  "settings.advanced.autostartAsAdmin.hideInWindowTitle.label",
                  "Hide Administrator from Window Title",
                )}
                description={t(
                  "settings.advanced.autostartAsAdmin.hideInWindowTitle.description",
                  "Do not show \"(Administrator)\" in the main window title. The tray menu always shows it.",
                )}
                descriptionMode={descriptionMode}
                grouped={grouped}
              />
            )}
            {autostartAsAdmin && (
              <div className="mx-4 mb-2 p-2.5 bg-amber-500/10 border border-amber-500/25 rounded-lg text-xs text-amber-200/90 leading-relaxed">
                <span className="font-semibold text-amber-300">
                  {t(
                    "settings.advanced.autostartAsAdmin.securityTitle",
                    "Security Notice:",
                  )}{" "}
                </span>
                {t(
                  "settings.advanced.autostartAsAdmin.securityNotice",
                  "Running AivoRelay permanently with administrator privileges grants all application components, plugins, and network integrations elevated system access. Enable this only if you need to dictate into elevated/admin programs.",
                )}
              </div>
            )}
          </>
        )}
      </>
    );
  },
);
