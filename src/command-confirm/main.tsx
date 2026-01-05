import React from "react";
import ReactDOM from "react-dom/client";
import CommandConfirmOverlay from "./CommandConfirmOverlay";
import "./CommandConfirmOverlay.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <CommandConfirmOverlay />
  </React.StrictMode>
);
