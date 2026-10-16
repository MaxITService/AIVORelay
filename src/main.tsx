import React from "react";
import ReactDOM from "react-dom/client";
import { Toaster } from "sonner";
import { platform } from "@tauri-apps/plugin-os";
import App from "./App";
import { RenderCrashBoundary } from "./components/RenderCrashBoundary";
import { installFrontendErrorLogging } from "./lib/frontendErrorLog";

// Set platform before render so CSS can scope per-platform (e.g. scrollbar styles)
document.documentElement.dataset.platform = platform();

// Initialize i18n
import "./i18n";

installFrontendErrorLogging();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Toaster
      theme="dark"
      toastOptions={{
        style: {
          background: "rgba(26, 26, 26, 0.98)",
          border: "1px solid #333333",
          color: "#f5f5f5",
          backdropFilter: "blur(12px)",
        },
      }}
    />
    <RenderCrashBoundary>
      <App />
    </RenderCrashBoundary>
  </React.StrictMode>,
);
