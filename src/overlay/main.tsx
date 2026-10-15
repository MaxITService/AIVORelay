import React from "react";
import ReactDOM from "react-dom/client";
import RecordingOverlay from "./RecordingOverlay";
import { SilentCrashBoundary } from "@/components/SilentCrashBoundary";
import { installFrontendErrorLogging } from "@/lib/frontendErrorLog";
import "@/i18n";

installFrontendErrorLogging();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SilentCrashBoundary where="window root">
      <RecordingOverlay />
    </SilentCrashBoundary>
  </React.StrictMode>,
);
