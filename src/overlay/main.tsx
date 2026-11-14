import React from "react";
import ReactDOM from "react-dom/client";
import RecordingOverlay from "./RecordingOverlay";
import { SilentCrashBoundary } from "@/components/SilentCrashBoundary";
import { installFrontendErrorLogging } from "@/lib/frontendErrorLog";
import "@/i18n";

installFrontendErrorLogging();

// The overlay floats over other apps, so a stray right-click must not open the
// webview's default menu (Reload, Save as, Print, ...). Kept in dev for Inspect.
if (import.meta.env.PROD) {
  window.addEventListener("contextmenu", (e) => e.preventDefault());
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SilentCrashBoundary where="window root">
      <RecordingOverlay />
    </SilentCrashBoundary>
  </React.StrictMode>,
);
