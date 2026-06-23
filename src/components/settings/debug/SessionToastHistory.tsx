import React, { useCallback, useMemo, useState } from "react";
import { AlertCircle, AlertTriangle } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useSessionToastStore } from "@/stores/sessionToastStore";
import { SettingContainer } from "../../ui/SettingContainer";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { ToggleSwitch } from "../../ui/ToggleSwitch";

const SESSION_TOAST_HISTORY_COLLAPSED_KEY =
  "aivorelay.debug.sessionToastHistory.collapsed";

const getInitialCollapsedState = (): boolean => {
  try {
    return (
      window.localStorage.getItem(SESSION_TOAST_HISTORY_COLLAPSED_KEY) ===
      "true"
    );
  } catch {
    return false;
  }
};

export const SessionToastHistory: React.FC = () => {
  const { t, i18n } = useTranslation();
  const [isCollapsed, setIsCollapsed] = useState(getInitialCollapsedState);
  const {
    toasts,
    showErrors,
    showWarnings,
    setShowErrors,
    setShowWarnings,
  } = useSessionToastStore();

  const errorCount = toasts.filter((toast) => toast.level === "error").length;
  const warningCount = toasts.length - errorCount;
  const visibleToasts = toasts
    .filter(
      (toast) =>
        (toast.level === "error" && showErrors) ||
        (toast.level === "warning" && showWarnings),
    )
    .reverse();

  const dateTimeFormatter = useMemo(
    () =>
      new Intl.DateTimeFormat(i18n.resolvedLanguage ?? i18n.language, {
        dateStyle: "short",
        timeStyle: "medium",
      }),
    [i18n.language, i18n.resolvedLanguage],
  );

  const updateCollapsed = useCallback((collapsed: boolean) => {
    setIsCollapsed(collapsed);
    try {
      window.localStorage.setItem(
        SESSION_TOAST_HISTORY_COLLAPSED_KEY,
        collapsed ? "true" : "false",
      );
    } catch {
      // UI preference only; keep the in-memory toggle working without storage.
    }
  }, []);

  return (
    <SettingsGroup
      title={t("settings.debug.sessionToasts.title", {
        count: toasts.length,
      })}
      description={
        isCollapsed
          ? undefined
          : t("settings.debug.sessionToasts.description")
      }
      collapsible={true}
      collapsed={isCollapsed}
      collapseLabel={t("settings.debug.sessionToasts.collapse")}
      expandLabel={t("settings.debug.sessionToasts.expand")}
      onCollapsedChange={updateCollapsed}
    >
      <SettingContainer
        title={t("settings.debug.sessionToasts.filters.errors", {
          count: errorCount,
        })}
        description={t(
          "settings.debug.sessionToasts.filters.errorsDescription",
        )}
        descriptionMode="inline"
        grouped={true}
      >
        <ToggleSwitch checked={showErrors} onChange={setShowErrors} />
      </SettingContainer>
      <SettingContainer
        title={t("settings.debug.sessionToasts.filters.warnings", {
          count: warningCount,
        })}
        description={t(
          "settings.debug.sessionToasts.filters.warningsDescription",
        )}
        descriptionMode="inline"
        grouped={true}
      >
        <ToggleSwitch checked={showWarnings} onChange={setShowWarnings} />
      </SettingContainer>

      {toasts.length === 0 ? (
        <div className="px-6 py-5 text-sm text-[#a0a0a0]">
          {t("settings.debug.sessionToasts.empty")}
        </div>
      ) : visibleToasts.length === 0 ? (
        <div className="px-6 py-5 text-sm text-[#a0a0a0]">
          {t("settings.debug.sessionToasts.filteredEmpty", {
            errorCount,
            warningCount,
          })}
        </div>
      ) : (
        <div className="divide-y divide-white/[0.05]">
          {visibleToasts.map((toast) => {
            const isError = toast.level === "error";
            const Icon = isError ? AlertCircle : AlertTriangle;

            return (
              <article key={toast.id} className="px-6 py-4">
                <div className="flex items-start gap-3">
                  <Icon
                    className={`mt-0.5 h-4 w-4 shrink-0 ${
                      isError ? "text-red-400" : "text-yellow-400"
                    }`}
                    aria-hidden="true"
                  />
                  <div className="min-w-0 flex-1">
                    <div className="mb-1.5 flex flex-wrap items-center gap-x-2 gap-y-1">
                      <span
                        className={`text-[10px] font-semibold uppercase tracking-wider ${
                          isError ? "text-red-300" : "text-yellow-300"
                        }`}
                      >
                        {t(
                          `settings.debug.sessionToasts.levels.${toast.level}`,
                        )}
                      </span>
                      <time
                        className="text-[11px] text-[#777]"
                        dateTime={new Date(toast.shownAt).toISOString()}
                      >
                        {dateTimeFormatter.format(toast.shownAt)}
                      </time>
                    </div>
                    {toast.message && (
                      <p className="whitespace-pre-wrap break-words text-sm font-medium text-[#e8e8e8] select-text">
                        {toast.message}
                      </p>
                    )}
                    {toast.description && (
                      <p className="mt-1.5 whitespace-pre-wrap break-words text-sm leading-relaxed text-[#b0b0b0] select-text">
                        {toast.description}
                      </p>
                    )}
                    {toast.actionLabel && (
                      <p className="mt-2 break-words text-xs text-[#888] select-text">
                        {t("settings.debug.sessionToasts.action", {
                          label: toast.actionLabel,
                        })}
                      </p>
                    )}
                  </div>
                </div>
              </article>
            );
          })}
        </div>
      )}
    </SettingsGroup>
  );
};
