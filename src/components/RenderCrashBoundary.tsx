import React from "react";
import { useTranslation } from "react-i18next";
import { commands } from "@/bindings";
import { describeError, logRenderCrash } from "@/lib/frontendErrorLog";
import { SilentCrashBoundary } from "./SilentCrashBoundary";
import { Button } from "./ui/Button";
import UpdateChecker from "./update-checker";

const RenderCrashFallback: React.FC<{ section?: string; error: unknown }> = ({
  section,
  error,
}) => {
  const { t } = useTranslation();
  const card = (
    <div
      role="alert"
      className="w-full max-w-xl rounded-lg border border-[#ff4757]/40 bg-[#1a1a1a] p-5 space-y-3 text-[#f5f5f5]"
    >
      <h2 className="text-base font-semibold">
        {t(section ? "errors.renderCrash.sectionTitle" : "errors.renderCrash.appTitle")}
      </h2>
      <p className="text-sm text-[#b8b8b8]">
        {t(section ? "errors.renderCrash.sectionDescription" : "errors.renderCrash.appDescription")}
      </p>
      <pre className="max-h-40 overflow-auto whitespace-pre-wrap break-words rounded bg-[#121212] p-3 text-xs text-[#ff6b7a]">
        {describeError(error)}
      </pre>
      <div className="flex flex-wrap gap-2">
        <Button size="sm" onClick={() => window.location.reload()}>
          {t("errors.renderCrash.reload")}
        </Button>
        <Button size="sm" variant="secondary" onClick={() => void commands.openLogDir()}>
          {t("errors.renderCrash.openLogs")}
        </Button>
      </div>
      {!section && (
        // The footer updater dies with the window, so offer it here: an
        // update is often the only fix for a crash that survives a reload.
        <div className="text-sm">
          <SilentCrashBoundary where="crash screen update checker">
            <UpdateChecker />
          </SilentCrashBoundary>
        </div>
      )}
    </div>
  );
  if (section) return card;
  return (
    <div className="h-screen flex items-center justify-center bg-[#121212] p-6">
      {card}
    </div>
  );
};

type RenderCrashBoundaryProps = {
  /** Settings section id; omit for the whole-window boundary. */
  section?: string;
  children: React.ReactNode;
};

type RenderCrashBoundaryState = { hasError: boolean; error: unknown };

/**
 * Without a boundary React unmounts the whole tree on a render error, which
 * leaves an empty, black main window. Key a section boundary by section so
 * navigating away and back retries it.
 */
export class RenderCrashBoundary extends React.Component<
  RenderCrashBoundaryProps,
  RenderCrashBoundaryState
> {
  state: RenderCrashBoundaryState = { hasError: false, error: null };

  static getDerivedStateFromError(error: unknown): RenderCrashBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: unknown, info: React.ErrorInfo) {
    const where = this.props.section
      ? `settings section "${this.props.section}"`
      : "window root";
    logRenderCrash(where, error, info.componentStack);
  }

  render() {
    if (this.state.hasError) {
      return <RenderCrashFallback section={this.props.section} error={this.state.error} />;
    }
    return this.props.children;
  }
}
