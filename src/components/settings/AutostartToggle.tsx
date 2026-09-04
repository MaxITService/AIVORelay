import React from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { type as getOsType } from "@tauri-apps/plugin-os";
import { sessionToast as toast } from "@/lib/sessionToast";
import { ToggleSwitch } from "../ui/ToggleSwitch";
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

export const AutostartToggle: React.FC<AutostartToggleProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating, refreshSettings } =
      useSettings();

    const autostartEnabled = getSetting("autostart_enabled") ?? false;
    const autostartAsAdmin =
      getSetting("autostart_as_admin_enabled") ?? false;
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
        toast.error(String(error));
      } finally {
        await refreshSettings();
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
