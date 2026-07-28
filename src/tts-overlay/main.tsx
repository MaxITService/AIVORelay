import React from "react";
import ReactDOM from "react-dom/client";
import TtsOverlay from "./TtsOverlay";
import "@/i18n";
import "./TtsOverlay.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <TtsOverlay />
  </React.StrictMode>,
);
