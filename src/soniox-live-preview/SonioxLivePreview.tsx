import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type SonioxLivePreviewPayload = {
  final_text?: string;
  interim_text?: string;
  finalText?: string;
  interimText?: string;
};

type SonioxLivePreviewAppearancePayload = {
  theme?: string;
  opacity_percent?: number;
  opacityPercent?: number;
  font_color?: string;
  fontColor?: string;
  interim_font_color?: string;
  interimFontColor?: string;
  accent_color?: string;
  accentColor?: string;
  interim_opacity_percent?: number;
  interimOpacityPercent?: number;
};

type SonioxLivePreviewAppearance = {
  theme: string;
  opacityPercent: number;
  fontColor: string;
  interimFontColor: string;
  accentColor: string;
  interimOpacityPercent: number;
};

type PreviewOutputModeStatePayload = {
  active?: boolean;
  recording?: boolean;
  processing_llm?: boolean;
  processingLlm?: boolean;
  flush_visible?: boolean;
  flushVisible?: boolean;
  is_realtime?: boolean;
  isRealtime?: boolean;
  error_message?: string | null;
  errorMessage?: string | null;
};

type PreviewOutputModeState = {
  active: boolean;
  recording: boolean;
  processingLlm: boolean;
  flushVisible: boolean;
  isRealtime: boolean;
  errorMessage: string | null;
};

type RgbTuple = [number, number, number];
type ThemePreset = {
  top: RgbTuple;
  bottom: RgbTuple;
  empty: RgbTuple;
};

const DEFAULT_APPEARANCE: SonioxLivePreviewAppearance = {
  theme: "main_dark",
  opacityPercent: 88,
  fontColor: "#f5f5f5",
  interimFontColor: "#f5f5f5",
  accentColor: "#ff4d8d",
  interimOpacityPercent: 58,
};

const DEFAULT_WORKFLOW_STATE: PreviewOutputModeState = {
  active: false,
  recording: false,
  processingLlm: false,
  flushVisible: false,
  isRealtime: false,
  errorMessage: null,
};

const THEME_PRESETS: Record<string, ThemePreset> = {
  // Matches main application palette.
  main_dark: {
    top: [26, 26, 26],
    bottom: [18, 18, 18],
    empty: [160, 160, 160],
  },
  ocean: {
    top: [9, 20, 37],
    bottom: [10, 30, 56],
    empty: [127, 153, 178],
  },
  light: {
    top: [244, 245, 248],
    bottom: [229, 231, 236],
    empty: [106, 114, 128],
  },
};

function parseHexColor(value: unknown, fallback: string): string {
  if (typeof value !== "string") {
    return fallback;
  }
  const trimmed = value.trim().toLowerCase();
  if (/^#[0-9a-f]{6}$/.test(trimmed)) {
    return trimmed;
  }
  return fallback;
}

function clampPercent(value: unknown, min: number, max: number, fallback: number): number {
  if (typeof value !== "number" || Number.isNaN(value)) {
    return fallback;
  }
  return Math.min(max, Math.max(min, Math.round(value)));
}

function hexToRgb(value: string, fallback: RgbTuple): RgbTuple {
  const normalized = parseHexColor(value, "");
  if (normalized.length !== 7) {
    return fallback;
  }
  const r = Number.parseInt(normalized.slice(1, 3), 16);
  const g = Number.parseInt(normalized.slice(3, 5), 16);
  const b = Number.parseInt(normalized.slice(5, 7), 16);
  if ([r, g, b].some((v) => Number.isNaN(v))) {
    return fallback;
  }
  return [r, g, b];
}

function rgba([r, g, b]: RgbTuple, alpha: number): string {
  return `rgba(${r}, ${g}, ${b}, ${alpha.toFixed(3)})`;
}

function srgbToLinear(channel: number): number {
  const normalized = channel / 255;
  if (normalized <= 0.04045) {
    return normalized / 12.92;
  }
  return ((normalized + 0.055) / 1.055) ** 2.4;
}

function relativeLuminance([r, g, b]: RgbTuple): number {
  const rl = srgbToLinear(r);
  const gl = srgbToLinear(g);
  const bl = srgbToLinear(b);
  return 0.2126 * rl + 0.7152 * gl + 0.0722 * bl;
}

function contrastRatio(a: RgbTuple, b: RgbTuple): number {
  const l1 = relativeLuminance(a);
  const l2 = relativeLuminance(b);
  const lighter = Math.max(l1, l2);
  const darker = Math.min(l1, l2);
  return (lighter + 0.05) / (darker + 0.05);
}

function ensureReadableTextColor(text: RgbTuple, bg: RgbTuple): RgbTuple {
  const minContrast = 2.8;
  if (contrastRatio(text, bg) >= minContrast) {
    return text;
  }

  const dark: RgbTuple = [24, 24, 27];
  const light: RgbTuple = [245, 245, 245];
  return contrastRatio(dark, bg) >= contrastRatio(light, bg) ? dark : light;
}

export default function SonioxLivePreview() {
  const [finalText, setFinalText] = useState("");
  const [interimText, setInterimText] = useState("");
  const [appearance, setAppearance] =
    useState<SonioxLivePreviewAppearance>(DEFAULT_APPEARANCE);
  const [workflowState, setWorkflowState] =
    useState<PreviewOutputModeState>(DEFAULT_WORKFLOW_STATE);
  const [isActionBusy, setIsActionBusy] = useState(false);
  const scrollRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const unlistenFns: Array<() => void> = [];
    let pollId: number | null = null;
    let active = true;

    const applyPayload = (raw: unknown) => {
      if (!active) {
        return;
      }

      let payload = raw;
      if (typeof payload === "string") {
        try {
          payload = JSON.parse(payload);
        } catch {
          return;
        }
      }

      if (!payload || typeof payload !== "object") {
        return;
      }

      const data = payload as SonioxLivePreviewPayload;
      const nextFinal =
        typeof data.final_text === "string"
          ? data.final_text
          : typeof data.finalText === "string"
            ? data.finalText
            : "";
      const nextInterim =
        typeof data.interim_text === "string"
          ? data.interim_text
          : typeof data.interimText === "string"
            ? data.interimText
            : "";

      setFinalText(nextFinal);
      setInterimText(nextInterim);
    };

    const applyAppearancePayload = (raw: unknown) => {
      if (!active) {
        return;
      }

      let payload = raw;
      if (typeof payload === "string") {
        try {
          payload = JSON.parse(payload);
        } catch {
          return;
        }
      }
      if (!payload || typeof payload !== "object") {
        return;
      }

      const data = payload as SonioxLivePreviewAppearancePayload;
      const theme =
        typeof data.theme === "string" && THEME_PRESETS[data.theme]
          ? data.theme
          : DEFAULT_APPEARANCE.theme;
      const opacityPercent = clampPercent(
        typeof data.opacity_percent === "number"
          ? data.opacity_percent
          : data.opacityPercent,
        35,
        100,
        DEFAULT_APPEARANCE.opacityPercent,
      );
      const fontColor = parseHexColor(
        typeof data.font_color === "string" ? data.font_color : data.fontColor,
        DEFAULT_APPEARANCE.fontColor,
      );
      const interimFontColor = parseHexColor(
        typeof data.interim_font_color === "string"
          ? data.interim_font_color
          : data.interimFontColor,
        DEFAULT_APPEARANCE.interimFontColor,
      );
      const accentColor = parseHexColor(
        typeof data.accent_color === "string"
          ? data.accent_color
          : data.accentColor,
        DEFAULT_APPEARANCE.accentColor,
      );
      const interimOpacityPercent = clampPercent(
        typeof data.interim_opacity_percent === "number"
          ? data.interim_opacity_percent
          : data.interimOpacityPercent,
        20,
        95,
        DEFAULT_APPEARANCE.interimOpacityPercent,
      );

      setAppearance({
        theme,
        opacityPercent,
        fontColor,
        interimFontColor,
        accentColor,
        interimOpacityPercent,
      });
    };

    const applyWorkflowPayload = (raw: unknown) => {
      if (!active) {
        return;
      }

      let payload = raw;
      if (typeof payload === "string") {
        try {
          payload = JSON.parse(payload);
        } catch {
          return;
        }
      }

      if (!payload || typeof payload !== "object") {
        return;
      }

      const data = payload as PreviewOutputModeStatePayload;
      setWorkflowState({
        active: Boolean(data.active),
        recording: Boolean(data.recording),
        processingLlm:
          typeof data.processing_llm === "boolean"
            ? data.processing_llm
            : Boolean(data.processingLlm),
        flushVisible:
          typeof data.flush_visible === "boolean"
            ? data.flush_visible
            : Boolean(data.flushVisible),
        isRealtime:
          typeof data.is_realtime === "boolean"
            ? data.is_realtime
            : Boolean(data.isRealtime),
        errorMessage:
          typeof data.error_message === "string"
            ? data.error_message
            : typeof data.errorMessage === "string"
              ? data.errorMessage
              : null,
      });
    };

    const refreshFromBackend = async () => {
      try {
        const payload = await invoke<SonioxLivePreviewPayload>(
          "get_soniox_live_preview_state",
        );
        applyPayload(payload);
      } catch {
        // Ignore poll errors to avoid noisy console loops.
      }
    };

    const refreshAppearanceFromBackend = async () => {
      try {
        const payload = await invoke<SonioxLivePreviewAppearancePayload>(
          "get_soniox_live_preview_appearance",
        );
        applyAppearancePayload(payload);
      } catch {
        // Ignore appearance polling errors.
      }
    };

    const refreshWorkflowFromBackend = async () => {
      try {
        const payload = await invoke<PreviewOutputModeStatePayload>(
          "get_preview_output_mode_state",
        );
        applyWorkflowPayload(payload);
      } catch {
        // Ignore workflow polling errors.
      }
    };

    const setup = async () => {
      const updateEvents = [
        "soniox-live-preview-update",
        "soniox_live_preview_update",
      ];
      const appearanceEvents = [
        "soniox-live-preview-appearance-update",
        "soniox_live_preview_appearance_update",
      ];
      const resetEvents = [
        "soniox-live-preview-reset",
        "soniox_live_preview_reset",
      ];
      const workflowEvents = [
        "preview-output-mode-state",
        "preview_output_mode_state",
      ];

      for (const eventName of updateEvents) {
        const unlistenApp = await listen<unknown>(eventName, (event) => {
          applyPayload(event.payload);
        });
        unlistenFns.push(unlistenApp);
      }

      for (const eventName of appearanceEvents) {
        const unlistenApp = await listen<unknown>(eventName, (event) => {
          applyAppearancePayload(event.payload);
        });
        unlistenFns.push(unlistenApp);
      }

      for (const eventName of resetEvents) {
        const resetHandler = () => {
          if (!active) {
            return;
          }
          setFinalText("");
          setInterimText("");
        };

        const unlistenApp = await listen(eventName, resetHandler);
        unlistenFns.push(unlistenApp);
      }

      for (const eventName of workflowEvents) {
        const unlistenApp = await listen<unknown>(eventName, (event) => {
          applyWorkflowPayload(event.payload);
        });
        unlistenFns.push(unlistenApp);
      }
    };

    void setup();
    void refreshFromBackend();
    void refreshAppearanceFromBackend();
    void refreshWorkflowFromBackend();
    pollId = window.setInterval(() => {
      void refreshFromBackend();
      void refreshWorkflowFromBackend();
    }, 120);

    return () => {
      active = false;
      for (const unlisten of unlistenFns) {
        unlisten();
      }
      if (pollId !== null) {
        window.clearInterval(pollId);
      }
    };
  }, []);

  const fullText = useMemo(
    () => `${finalText}${interimText}`,
    [finalText, interimText],
  );
  const hasText = useMemo(() => fullText.trim().length > 0, [fullText]);
  const actionLocked = workflowState.processingLlm || isActionBusy;
  const canRunTextActions = hasText || workflowState.recording;
  const emptyStateMessage = useMemo(() => {
    if (
      workflowState.active &&
      workflowState.recording &&
      !workflowState.isRealtime
    ) {
      return "Recording... text appears after stop/flush in non-realtime mode.";
    }
    return "Waiting for speech...";
  }, [
    workflowState.active,
    workflowState.isRealtime,
    workflowState.recording,
  ]);

  const rootStyle = useMemo(() => {
    const preset = THEME_PRESETS[appearance.theme] ?? THEME_PRESETS.main_dark;
    const panelAlpha = appearance.opacityPercent / 100;
    const interimAlpha = appearance.interimOpacityPercent / 100;
    const panelBase: RgbTuple = [
      Math.round((preset.top[0] + preset.bottom[0]) / 2),
      Math.round((preset.top[1] + preset.bottom[1]) / 2),
      Math.round((preset.top[2] + preset.bottom[2]) / 2),
    ];
    const fontRgb = ensureReadableTextColor(
      hexToRgb(appearance.fontColor, [245, 245, 245]),
      panelBase,
    );
    const interimFontRgb = ensureReadableTextColor(
      hexToRgb(appearance.interimFontColor, [245, 245, 245]),
      panelBase,
    );
    const accentRgb = hexToRgb(appearance.accentColor, [255, 77, 141]);

    return {
      "--slp-bg-top": rgba(preset.top, panelAlpha),
      "--slp-bg-bottom": rgba(preset.bottom, panelAlpha),
      "--slp-border-color": rgba(accentRgb, 0.45),
      "--slp-shadow-color": rgba(accentRgb, 0.2),
      "--slp-final-color": rgba(fontRgb, 1),
      "--slp-interim-color": rgba(interimFontRgb, interimAlpha),
      "--slp-empty-color": rgba(preset.empty, 1),
    } as CSSProperties;
  }, [appearance]);

  useEffect(() => {
    const element = scrollRef.current;
    if (!element) {
      return;
    }
    element.scrollTop = element.scrollHeight;
  }, [fullText]);

  const invokePreviewAction = async (command: string) => {
    setIsActionBusy(true);
    try {
      await invoke(command);
    } catch (error) {
      console.error(`Preview command failed: ${command}`, error);
    } finally {
      setIsActionBusy(false);
    }
  };

  const handleClose = () => {
    void invokePreviewAction("preview_close_action");
  };

  const handleClear = () => {
    void invokePreviewAction("preview_clear_action");
  };

  const handleInsert = () => {
    void invokePreviewAction("preview_insert_action");
  };

  const handleProcess = () => {
    void invokePreviewAction("preview_llm_process_action");
  };

  const handleFlush = () => {
    void invokePreviewAction("preview_flush_action");
  };

  return (
    <div className="soniox-live-preview-root" style={rootStyle}>
      <div className="soniox-live-preview-header">
        <div className="soniox-live-preview-title">
          {workflowState.active ? "Output Only to Preview" : "Live Preview"}
        </div>
        <button
          type="button"
          className="soniox-live-preview-close"
          onClick={handleClose}
          title="Close"
        >
          X
        </button>
      </div>
      <div className="soniox-live-preview-body" ref={scrollRef}>
        {fullText.length === 0 ? (
          <span className="soniox-live-preview-empty">{emptyStateMessage}</span>
        ) : (
          <>
            <span className="soniox-live-preview-final">{finalText}</span>
            <span className="soniox-live-preview-interim">{interimText}</span>
          </>
        )}
      </div>
      {workflowState.errorMessage && (
        <div className="soniox-live-preview-error">{workflowState.errorMessage}</div>
      )}
      {workflowState.active && (
        <div className="soniox-live-preview-actions">
          <button
            type="button"
            className="soniox-live-preview-action-button"
            onClick={handleClear}
            disabled={actionLocked || !hasText}
          >
            Clear all
          </button>
          {workflowState.flushVisible && (
            <button
              type="button"
              className="soniox-live-preview-action-button"
              onClick={handleFlush}
              disabled={actionLocked || !canRunTextActions}
            >
              Flush
            </button>
          )}
          <button
            type="button"
            className="soniox-live-preview-action-button"
            onClick={handleProcess}
            disabled={actionLocked || !canRunTextActions}
          >
            {workflowState.processingLlm ? "Processing..." : "Processing via LLM"}
          </button>
          <button
            type="button"
            className="soniox-live-preview-action-button soniox-live-preview-action-button-primary"
            onClick={handleInsert}
            disabled={actionLocked || (!hasText && !workflowState.recording)}
          >
            Insert
          </button>
        </div>
      )}
    </div>
  );
}
