import React from "react";
import ReactDOM from "react-dom/client";
import RegionCaptureOverlay from "./RegionCaptureOverlay";
import { invoke } from "@tauri-apps/api/core";
import { SilentCrashBoundary } from "@/components/SilentCrashBoundary";
import { installFrontendErrorLogging } from "@/lib/frontendErrorLog";
import "./RegionCaptureOverlay.css";

installFrontendErrorLogging();

// A blank picker would cover the screen with Escape no longer handled.
const cancelCapture = () => void invoke("region_capture_cancel").catch(() => {});

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <SilentCrashBoundary where="window root" onCrash={cancelCapture}>
      <RegionCaptureOverlay />
    </SilentCrashBoundary>
  </React.StrictMode>
);
