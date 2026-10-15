import React from "react";
import ReactDOM from "react-dom/client";
import CommandConfirmOverlay from "./CommandConfirmOverlay";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { SilentCrashBoundary } from "@/components/SilentCrashBoundary";
import { installFrontendErrorLogging } from "@/lib/frontendErrorLog";
import "./CommandConfirmOverlay.css";

installFrontendErrorLogging();

// A blank confirm popup would linger with nothing to click.
const closeWindow = () => void getCurrentWindow().destroy().catch(() => {});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SilentCrashBoundary where="window root" onCrash={closeWindow}>
      <CommandConfirmOverlay />
    </SilentCrashBoundary>
  </React.StrictMode>
);
