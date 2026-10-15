import React from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { commands } from "@/bindings";
import { Button } from "./ui/Button";
import UpdateChecker from "./update-checker";

const describeError = (error: unknown) =>
  error instanceof Error ? `${error.name}: ${error.message}` : String(error);

// tauri-plugin-log's own command (level 5 = Error), so render crashes reach
// aivorelay.log instead of existing only in the WebView2 DevTools console.
const logRenderCrash = (
  where: string,
  error: unknown,
  componentStack?: string | null,
) => {
  const stack = error instanceof Error && error.stack ? `\n${error.stack}` : "";
  const components = componentStack ? `\nComponent stack:${componentStack}` : "";
  void invoke("plugin:log|log", {
    level: 5,
    message: `Render crash in ${where}: ${describeError(error)}${stack}${components}`,
    location: "render-crash",
  }).catch(() => {});
};

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
          <QuietBoundary>
            <UpdateChecker />
          </QuietBoundary>
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

/** Renders nothing when an optional crash-screen extra crashes too. */
class QuietBoundary extends React.Component<
  { children: React.ReactNode },
  { hasError: boolean }
> {
  state = { hasError: false };

  static getDerivedStateFromError() {
    return { hasError: true };
  }

  componentDidCatch(error: unknown, info: React.ErrorInfo) {
    logRenderCrash("crash screen extra", error, info.componentStack);
  }

  render() {
    return this.state.hasError ? null : this.props.children;
  }
}

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
      : "main window";
    logRenderCrash(where, error, info.componentStack);
  }

  render() {
    if (this.state.hasError) {
      return <RenderCrashFallback section={this.props.section} error={this.state.error} />;
    }
    return this.props.children;
  }
}
