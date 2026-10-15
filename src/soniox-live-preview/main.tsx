import React from "react";
import ReactDOM from "react-dom/client";
import SonioxLivePreview from "./SonioxLivePreview";
import { SilentCrashBoundary } from "@/components/SilentCrashBoundary";
import { installFrontendErrorLogging } from "@/lib/frontendErrorLog";
import "./SonioxLivePreview.css";

installFrontendErrorLogging();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SilentCrashBoundary where="window root">
      <SonioxLivePreview />
    </SilentCrashBoundary>
  </React.StrictMode>,
);
