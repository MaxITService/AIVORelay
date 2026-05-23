import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSettings } from "../../hooks/useSettings";
import { GlobalShortcutInput } from "./GlobalShortcutInput";
import { HandyKeysShortcutInput } from "./HandyKeysShortcutInput";
import { getShortcutAnchorId } from "@/lib/shortcutAnchors";

interface HandyShortcutProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  shortcutId: string;
  disabled?: boolean;
}

export const HandyShortcut: React.FC<HandyShortcutProps> = (props) => {
  const { getSetting } = useSettings();
  const configuredEngine =
    (getSetting("shortcut_engine") as string) ?? "handy_keys";
  const [activeEngine, setActiveEngine] = useState(configuredEngine);
  const shortcutInput =
    activeEngine === "handy_keys" ? (
      <HandyKeysShortcutInput {...props} />
    ) : (
      <GlobalShortcutInput {...props} />
    );

  useEffect(() => {
    invoke<string>("get_current_shortcut_engine")
      .then((engine) => setActiveEngine(engine))
      .catch(() => setActiveEngine(configuredEngine));
  }, [configuredEngine]);

  return (
    <div
      id={getShortcutAnchorId(props.shortcutId)}
      className="shortcut-settings-anchor"
      data-shortcut-id={props.shortcutId}
    >
      {shortcutInput}
    </div>
  );
};
