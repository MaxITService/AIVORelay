import React from "react";
import ReactDOM from "react-dom/client";
import TtsOverlay from "./TtsOverlay";
import { SilentCrashBoundary } from "@/components/SilentCrashBoundary";
import { installFrontendErrorLogging } from "@/lib/frontendErrorLog";
import "@/i18n";
import "./TtsOverlay.css";

installFrontendErrorLogging();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SilentCrashBoundary where="window root">
      <TtsOverlay />
    </SilentCrashBoundary>
  </React.StrictMode>,
);
