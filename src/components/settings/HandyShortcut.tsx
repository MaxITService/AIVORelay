import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useSettings } from "../../hooks/useSettings";
import { GlobalShortcutInput } from "./GlobalShortcutInput";
import { HandyKeysShortcutInput } from "./HandyKeysShortcutInput";
import { getShortcutAnchorId } from "@/lib/shortcutAnchors";

interface HandyShortcutProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  shortcutId: string;
  disabled?: boolean;
  title?: string;
  description?: string;
  draftBinding?: string;
  onDraftBindingChange?: (binding: string) => void;
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
    let disposed = false;
    const refresh = () => {
      void invoke<string>("get_current_shortcut_engine")
        .then((engine) => { if (!disposed) setActiveEngine(engine); })
        .catch(() => { if (!disposed) setActiveEngine(configuredEngine); });
    };
    const unlisten = listen("shortcuts-initialized", refresh);
    refresh();
    return () => { disposed = true; void unlisten.then((stop) => stop()).catch(() => undefined); };
  }, [configuredEngine]);

  return (
    <div
      id={getShortcutAnchorId(props.shortcutId)}
      className="shortcut-settings-anchor"
      tabIndex={-1}
      data-shortcut-id={props.shortcutId}
    >
      {shortcutInput}
    </div>
  );
};
