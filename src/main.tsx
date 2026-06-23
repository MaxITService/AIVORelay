import React from "react";
import ReactDOM from "react-dom/client";
import { Toaster } from "sonner";
import App from "./App";

// Initialize i18n
import "./i18n";

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
    <App />
  </React.StrictMode>,
);
