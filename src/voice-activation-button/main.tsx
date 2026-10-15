import React from "react";
import ReactDOM from "react-dom/client";
import VoiceActivationButton from "./VoiceActivationButton";
import { SilentCrashBoundary } from "@/components/SilentCrashBoundary";
import { installFrontendErrorLogging } from "@/lib/frontendErrorLog";
import "./VoiceActivationButton.css";

installFrontendErrorLogging();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SilentCrashBoundary where="window root">
      <VoiceActivationButton />
    </SilentCrashBoundary>
  </React.StrictMode>,
);
