import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSettings } from "../../hooks/useSettings";
import { GlobalShortcutInput } from "./GlobalShortcutInput";
import { HandyKeysShortcutInput } from "./HandyKeysShortcutInput";

interface HandyShortcutProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  shortcutId: string;
  disabled?: boolean;
}

export const HandyShortcut: React.FC<HandyShortcutProps> = (props) => {
  const { getSetting } = useSettings();
  const configuredEngine = (getSetting("shortcut_engine") as string) ?? "tauri";
  const [activeEngine, setActiveEngine] = useState(configuredEngine);

  useEffect(() => {
    invoke<string>("get_current_shortcut_engine")
      .then((engine) => setActiveEngine(engine))
      .catch(() => setActiveEngine(configuredEngine));
  }, [configuredEngine]);

  if (activeEngine === "handy_keys") {
    return <HandyKeysShortcutInput {...props} />;
  }

  return <GlobalShortcutInput {...props} />;
};
