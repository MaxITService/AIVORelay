import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

// tauri-plugin-log levels (repr u16).
const LOG_LEVEL_WARN = 4;
const LOG_LEVEL_ERROR = 5;
const MAX_LOGGED_UNCAUGHT = 50;

export const describeError = (error: unknown) =>
  error instanceof Error ? `${error.name}: ${error.message}` : String(error);

const windowLabel = () => {
  try {
    return getCurrentWindow().label;
  } catch {
    return "unknown";
  }
};

// tauri-plugin-log's own command, so frontend failures reach aivorelay.log
// instead of existing only in the WebView2 DevTools console.
export const logFrontendError = (
  what: string,
  error: unknown,
  details = "",
  level = LOG_LEVEL_ERROR,
) => {
  // A V8 stack already starts with "Name: message".
  const text = error instanceof Error && error.stack ? error.stack : describeError(error);
  void invoke("plugin:log|log", {
    level,
    message: `[${windowLabel()} window] ${what}: ${text}${details}`,
    location: "frontend",
  }).catch(() => {});
};

export const logRenderCrash = (
  where: string,
  error: unknown,
  componentStack?: string | null,
) => {
  const components = componentStack ? `\nComponent stack:${componentStack}` : "";
  logFrontendError(`Render crash in ${where}`, error, components);
};

const loggedUncaught = new Set<string>();

// Each distinct failure is logged once, so one failing in a loop cannot flood the log.
const logUncaught = (what: string, error: unknown, level: number) => {
  const key = `${what}:${describeError(error)}`;
  if (loggedUncaught.has(key) || loggedUncaught.size >= MAX_LOGGED_UNCAUGHT) return;
  loggedUncaught.add(key);
  logFrontendError(what, error, "", level);
};

/** Logs failures no try/catch or error boundary handled. Call once per window entrypoint. */
export const installFrontendErrorLogging = () => {
  window.addEventListener("error", (event) => {
    logUncaught("Uncaught error", event.error ?? event.message, LOG_LEVEL_ERROR);
  });
  window.addEventListener("unhandledrejection", (event) => {
    logUncaught("Unhandled promise rejection", event.reason, LOG_LEVEL_WARN);
  });
};
