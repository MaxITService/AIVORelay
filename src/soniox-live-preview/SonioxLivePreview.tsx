import {
  useCallback,
  useEffect,
  useMemo,
  type PointerEvent,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { type as getOsType } from "@tauri-apps/plugin-os";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { OSType } from "../lib/utils/keyboard";
import {
  buildPreviewHotkeyFromKeyboardEvent,
  formatPreviewHotkeyForDisplay,
  normalizePreviewHotkeyString,
} from "../lib/utils/previewHotkeys";

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
  close_hotkey?: string;
  closeHotkey?: string;
  clear_hotkey?: string;
  clearHotkey?: string;
  flush_hotkey?: string;
  flushHotkey?: string;
  process_hotkey?: string;
  processHotkey?: string;
  insert_hotkey?: string;
  insertHotkey?: string;
  delete_until_dot_or_comma_hotkey?: string;
  deleteUntilDotOrCommaHotkey?: string;
  delete_until_dot_hotkey?: string;
  deleteUntilDotHotkey?: string;
  delete_last_word_hotkey?: string;
  deleteLastWordHotkey?: string;
  show_clear_button?: boolean;
  showClearButton?: boolean;
  show_flush_button?: boolean;
  showFlushButton?: boolean;
  show_process_button?: boolean;
  showProcessButton?: boolean;
  show_insert_button?: boolean;
  showInsertButton?: boolean;
  show_delete_until_dot_or_comma_button?: boolean;
  showDeleteUntilDotOrCommaButton?: boolean;
  show_delete_until_dot_button?: boolean;
  showDeleteUntilDotButton?: boolean;
  show_delete_last_word_button?: boolean;
  showDeleteLastWordButton?: boolean;
  ctrl_backspace_delete_last_word?: boolean;
  ctrlBackspaceDeleteLastWord?: boolean;
  backspace_delete_last_char?: boolean;
  backspaceDeleteLastChar?: boolean;
  show_drag_grip?: boolean;
  showDragGrip?: boolean;
};

type SonioxLivePreviewAppearance = {
  theme: string;
  opacityPercent: number;
  fontColor: string;
  interimFontColor: string;
  accentColor: string;
  interimOpacityPercent: number;
  closeHotkey: string;
  clearHotkey: string;
  flushHotkey: string;
  processHotkey: string;
  insertHotkey: string;
  deleteUntilDotOrCommaHotkey: string;
  deleteUntilDotHotkey: string;
  deleteLastWordHotkey: string;
  showClearButton: boolean;
  showFlushButton: boolean;
  showProcessButton: boolean;
  showInsertButton: boolean;
  showDeleteUntilDotOrCommaButton: boolean;
  showDeleteUntilDotButton: boolean;
  showDeleteLastWordButton: boolean;
  ctrlBackspaceDeleteLastWord: boolean;
  backspaceDeleteLastChar: boolean;
  showDragGrip: boolean;
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
  closeHotkey: "",
  clearHotkey: "",
  flushHotkey: "",
  processHotkey: "",
  insertHotkey: "",
  deleteUntilDotOrCommaHotkey: "",
  deleteUntilDotHotkey: "",
  deleteLastWordHotkey: "",
  showClearButton: true,
  showFlushButton: true,
  showProcessButton: true,
  showInsertButton: true,
  showDeleteUntilDotOrCommaButton: true,
  showDeleteUntilDotButton: true,
  showDeleteLastWordButton: true,
  ctrlBackspaceDeleteLastWord: true,
  backspaceDeleteLastChar: true,
  showDragGrip: true,
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

const windowRef = getCurrentWindow();
type PreviewResizeDirection = Parameters<typeof windowRef.startResizeDragging>[0];

const RESIZE_HANDLES: ReadonlyArray<{
  direction: PreviewResizeDirection;
  className: string;
}> = [
  { direction: "North", className: "soniox-live-preview-resize-handle soniox-live-preview-resize-handle-n" },
  { direction: "South", className: "soniox-live-preview-resize-handle soniox-live-preview-resize-handle-s" },
  { direction: "West", className: "soniox-live-preview-resize-handle soniox-live-preview-resize-handle-w" },
  { direction: "East", className: "soniox-live-preview-resize-handle soniox-live-preview-resize-handle-e" },
  { direction: "NorthWest", className: "soniox-live-preview-resize-handle soniox-live-preview-resize-handle-nw" },
  { direction: "NorthEast", className: "soniox-live-preview-resize-handle soniox-live-preview-resize-handle-ne" },
  { direction: "SouthWest", className: "soniox-live-preview-resize-handle soniox-live-preview-resize-handle-sw" },
  { direction: "SouthEast", className: "soniox-live-preview-resize-handle soniox-live-preview-resize-handle-se" },
];

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
  const osKind = getOsType();
  const osType: OSType =
    osKind === "windows" || osKind === "macos" || osKind === "linux"
      ? osKind
      : "unknown";
  const [finalText, setFinalText] = useState("");
  const [interimText, setInterimText] = useState("");
  const [appearance, setAppearance] =
    useState<SonioxLivePreviewAppearance>(DEFAULT_APPEARANCE);
  const [workflowState, setWorkflowState] =
    useState<PreviewOutputModeState>(DEFAULT_WORKFLOW_STATE);
  const [isActionBusy, setIsActionBusy] = useState(false);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const dragGripStateRef = useRef<{
    armed: boolean;
    sawMove: boolean;
    saveTimer: number | null;
  }>({
    armed: false,
    sawMove: false,
    saveTimer: null,
  });
  const resizeStateRef = useRef<{
    armed: boolean;
    sawResize: boolean;
    armTimer: number | null;
    saveTimer: number | null;
  }>({
    armed: false,
    sawResize: false,
    armTimer: null,
    saveTimer: null,
  });
  // When the user clears while a transcription is still in-flight, we set this
  // flag so that any subsequent update events are discarded until the backend
  // signals a new session (via the reset event).
  const ignoredUntilNextResetRef = useRef(false);

  useEffect(() => {
    const unlistenFns: Array<() => void> = [];
    let pollId: number | null = null;
    let active = true;

    const applyPayload = (raw: unknown) => {
      if (!active) {
        return;
      }
      if (ignoredUntilNextResetRef.current) {
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
      const closeHotkey = normalizePreviewHotkeyString(
        typeof data.close_hotkey === "string"
          ? data.close_hotkey
          : typeof data.closeHotkey === "string"
            ? data.closeHotkey
            : "",
      );
      const clearHotkey = normalizePreviewHotkeyString(
        typeof data.clear_hotkey === "string"
          ? data.clear_hotkey
          : typeof data.clearHotkey === "string"
            ? data.clearHotkey
            : "",
      );
      const flushHotkey = normalizePreviewHotkeyString(
        typeof data.flush_hotkey === "string"
          ? data.flush_hotkey
          : typeof data.flushHotkey === "string"
            ? data.flushHotkey
            : "",
      );
      const processHotkey = normalizePreviewHotkeyString(
        typeof data.process_hotkey === "string"
          ? data.process_hotkey
          : typeof data.processHotkey === "string"
            ? data.processHotkey
            : "",
      );
      const insertHotkey = normalizePreviewHotkeyString(
        typeof data.insert_hotkey === "string"
          ? data.insert_hotkey
          : typeof data.insertHotkey === "string"
            ? data.insertHotkey
            : "",
      );
      const deleteUntilDotOrCommaHotkey = normalizePreviewHotkeyString(
        typeof data.delete_until_dot_or_comma_hotkey === "string"
          ? data.delete_until_dot_or_comma_hotkey
          : typeof data.deleteUntilDotOrCommaHotkey === "string"
            ? data.deleteUntilDotOrCommaHotkey
            : "",
      );
      const deleteUntilDotHotkey = normalizePreviewHotkeyString(
        typeof data.delete_until_dot_hotkey === "string"
          ? data.delete_until_dot_hotkey
          : typeof data.deleteUntilDotHotkey === "string"
            ? data.deleteUntilDotHotkey
            : "",
      );
      const deleteLastWordHotkey = normalizePreviewHotkeyString(
        typeof data.delete_last_word_hotkey === "string"
          ? data.delete_last_word_hotkey
          : typeof data.deleteLastWordHotkey === "string"
            ? data.deleteLastWordHotkey
            : "",
      );
      const showClearButton =
        typeof data.show_clear_button === "boolean"
          ? data.show_clear_button
          : typeof data.showClearButton === "boolean"
            ? data.showClearButton
            : DEFAULT_APPEARANCE.showClearButton;
      const showFlushButton =
        typeof data.show_flush_button === "boolean"
          ? data.show_flush_button
          : typeof data.showFlushButton === "boolean"
            ? data.showFlushButton
            : DEFAULT_APPEARANCE.showFlushButton;
      const showProcessButton =
        typeof data.show_process_button === "boolean"
          ? data.show_process_button
          : typeof data.showProcessButton === "boolean"
            ? data.showProcessButton
            : DEFAULT_APPEARANCE.showProcessButton;
      const showInsertButton =
        typeof data.show_insert_button === "boolean"
          ? data.show_insert_button
          : typeof data.showInsertButton === "boolean"
            ? data.showInsertButton
            : DEFAULT_APPEARANCE.showInsertButton;
      const showDeleteUntilDotOrCommaButton =
        typeof data.show_delete_until_dot_or_comma_button === "boolean"
          ? data.show_delete_until_dot_or_comma_button
          : typeof data.showDeleteUntilDotOrCommaButton === "boolean"
            ? data.showDeleteUntilDotOrCommaButton
            : DEFAULT_APPEARANCE.showDeleteUntilDotOrCommaButton;
      const showDeleteUntilDotButton =
        typeof data.show_delete_until_dot_button === "boolean"
          ? data.show_delete_until_dot_button
          : typeof data.showDeleteUntilDotButton === "boolean"
            ? data.showDeleteUntilDotButton
            : DEFAULT_APPEARANCE.showDeleteUntilDotButton;
      const showDeleteLastWordButton =
        typeof data.show_delete_last_word_button === "boolean"
          ? data.show_delete_last_word_button
          : typeof data.showDeleteLastWordButton === "boolean"
            ? data.showDeleteLastWordButton
            : DEFAULT_APPEARANCE.showDeleteLastWordButton;
      const ctrlBackspaceDeleteLastWord =
        typeof data.ctrl_backspace_delete_last_word === "boolean"
          ? data.ctrl_backspace_delete_last_word
          : typeof data.ctrlBackspaceDeleteLastWord === "boolean"
            ? data.ctrlBackspaceDeleteLastWord
            : DEFAULT_APPEARANCE.ctrlBackspaceDeleteLastWord;
      const backspaceDeleteLastChar =
        typeof data.backspace_delete_last_char === "boolean"
          ? data.backspace_delete_last_char
          : typeof data.backspaceDeleteLastChar === "boolean"
            ? data.backspaceDeleteLastChar
            : DEFAULT_APPEARANCE.backspaceDeleteLastChar;
      const showDragGrip =
        typeof data.show_drag_grip === "boolean"
          ? data.show_drag_grip
          : typeof data.showDragGrip === "boolean"
            ? data.showDragGrip
            : DEFAULT_APPEARANCE.showDragGrip;

      setAppearance({
        theme,
        opacityPercent,
        fontColor,
        interimFontColor,
        accentColor,
        interimOpacityPercent,
        closeHotkey,
        clearHotkey,
        flushHotkey,
        processHotkey,
        insertHotkey,
        deleteUntilDotOrCommaHotkey,
        deleteUntilDotHotkey,
        deleteLastWordHotkey,
        showClearButton,
        showFlushButton,
        showProcessButton,
        showInsertButton,
        showDeleteUntilDotOrCommaButton,
        showDeleteUntilDotButton,
        showDeleteLastWordButton,
        ctrlBackspaceDeleteLastWord,
        backspaceDeleteLastChar,
        showDragGrip,
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
          ignoredUntilNextResetRef.current = false;
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
  const canClear = !actionLocked && hasText;
  const canDelete = !actionLocked && canRunTextActions;
  const canFlush = !actionLocked && workflowState.flushVisible && canRunTextActions;
  const canProcess = !actionLocked && canRunTextActions;
  const canInsert = !actionLocked && (hasText || workflowState.recording);
  const closeHotkeyLabel = useMemo(
    () => formatPreviewHotkeyForDisplay(appearance.closeHotkey, osType),
    [appearance.closeHotkey, osType],
  );
  const clearHotkeyLabel = useMemo(
    () => formatPreviewHotkeyForDisplay(appearance.clearHotkey, osType),
    [appearance.clearHotkey, osType],
  );
  const flushHotkeyLabel = useMemo(
    () => formatPreviewHotkeyForDisplay(appearance.flushHotkey, osType),
    [appearance.flushHotkey, osType],
  );
  const processHotkeyLabel = useMemo(
    () => formatPreviewHotkeyForDisplay(appearance.processHotkey, osType),
    [appearance.processHotkey, osType],
  );
  const insertHotkeyLabel = useMemo(
    () => formatPreviewHotkeyForDisplay(appearance.insertHotkey, osType),
    [appearance.insertHotkey, osType],
  );
  const deleteUntilDotOrCommaHotkeyLabel = useMemo(
    () =>
      formatPreviewHotkeyForDisplay(
        appearance.deleteUntilDotOrCommaHotkey,
        osType,
      ),
    [appearance.deleteUntilDotOrCommaHotkey, osType],
  );
  const deleteUntilDotHotkeyLabel = useMemo(
    () => formatPreviewHotkeyForDisplay(appearance.deleteUntilDotHotkey, osType),
    [appearance.deleteUntilDotHotkey, osType],
  );
  const deleteLastWordHotkeyLabel = useMemo(
    () => formatPreviewHotkeyForDisplay(appearance.deleteLastWordHotkey, osType),
    [appearance.deleteLastWordHotkey, osType],
  );
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

  useEffect(() => {
    let unlistenMoved: (() => void) | null = null;
    let unlistenResized: (() => void) | null = null;

    const setup = async () => {
      try {
        unlistenMoved = await windowRef.onMoved(({ payload }) => {
          const dragState = dragGripStateRef.current;
          if (!dragState.armed) {
            return;
          }

          dragState.sawMove = true;
          if (dragState.saveTimer !== null) {
            window.clearTimeout(dragState.saveTimer);
          }

          dragState.saveTimer = window.setTimeout(async () => {
            try {
              const scaleFactor = await windowRef.scaleFactor();
              const logicalX = Math.round(payload.x / scaleFactor);
              const logicalY = Math.round(payload.y / scaleFactor);
              await invoke("remember_soniox_live_preview_window_position", {
                xPx: logicalX,
                yPx: logicalY,
              });
            } catch (error) {
              console.error("Failed to persist live preview window position:", error);
            } finally {
              dragState.armed = false;
              dragState.sawMove = false;
              dragState.saveTimer = null;
            }
          }, 180);
        });
      } catch (error) {
        console.error("Failed to subscribe to live preview move events:", error);
      }

      try {
        unlistenResized = await windowRef.onResized(({ payload }) => {
          const resizeState = resizeStateRef.current;
          if (!resizeState.armed) {
            return;
          }

          resizeState.sawResize = true;
          if (resizeState.armTimer !== null) {
            window.clearTimeout(resizeState.armTimer);
            resizeState.armTimer = null;
          }
          if (resizeState.saveTimer !== null) {
            window.clearTimeout(resizeState.saveTimer);
          }

          resizeState.saveTimer = window.setTimeout(async () => {
            try {
              const scaleFactor = await windowRef.scaleFactor();
              const logicalSize = payload.toLogical(scaleFactor);
              const physicalPosition = await windowRef.innerPosition();
              const logicalPosition = physicalPosition.toLogical(scaleFactor);

              await Promise.all([
                invoke("remember_soniox_live_preview_window_position", {
                  xPx: Math.round(logicalPosition.x),
                  yPx: Math.round(logicalPosition.y),
                }),
                invoke("remember_soniox_live_preview_window_size", {
                  widthPx: Math.round(logicalSize.width),
                  heightPx: Math.round(logicalSize.height),
                }),
              ]);
            } catch (error) {
              console.error("Failed to persist live preview window geometry:", error);
            } finally {
              resizeState.armed = false;
              resizeState.sawResize = false;
              resizeState.saveTimer = null;
            }
          }, 180);
        });
      } catch (error) {
        console.error("Failed to subscribe to live preview resize events:", error);
      }
    };

    void setup();

    return () => {
      const dragState = dragGripStateRef.current;
      if (dragState.saveTimer !== null) {
        window.clearTimeout(dragState.saveTimer);
        dragState.saveTimer = null;
      }
      dragState.armed = false;
      dragState.sawMove = false;
      const resizeState = resizeStateRef.current;
      if (resizeState.armTimer !== null) {
        window.clearTimeout(resizeState.armTimer);
        resizeState.armTimer = null;
      }
      if (resizeState.saveTimer !== null) {
        window.clearTimeout(resizeState.saveTimer);
        resizeState.saveTimer = null;
      }
      resizeState.armed = false;
      resizeState.sawResize = false;
      if (unlistenMoved) {
        unlistenMoved();
      }
      if (unlistenResized) {
        unlistenResized();
      }
    };
  }, []);

  const labelWithHotkey = (label: string, hotkeyLabel: string): string =>
    hotkeyLabel ? `${label} (${hotkeyLabel})` : label;

  const invokePreviewAction = useCallback(async (command: string) => {
    setIsActionBusy(true);
    try {
      await invoke(command);
    } catch (error) {
      console.error(`Preview command failed: ${command}`, error);
    } finally {
      setIsActionBusy(false);
    }
  }, []);

  const handleClose = () => {
    void invokePreviewAction("preview_close_action");
  };

  const handleClear = () => {
    ignoredUntilNextResetRef.current = true;
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

  const handleDeleteUntilDotOrComma = () => {
    void invokePreviewAction("preview_delete_until_dot_or_comma_action");
  };

  const handleDeleteUntilDot = () => {
    void invokePreviewAction("preview_delete_until_dot_action");
  };

  const handleDeleteLastWord = () => {
    void invokePreviewAction("preview_delete_last_word_action");
  };

  const handleDragGripPointerDown = (event: PointerEvent<HTMLButtonElement>) => {
    if (event.button !== 0) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    const dragState = dragGripStateRef.current;
    dragState.armed = true;
    dragState.sawMove = false;
    if (dragState.saveTimer !== null) {
      window.clearTimeout(dragState.saveTimer);
      dragState.saveTimer = null;
    }

    void windowRef.startDragging().catch((error) => {
      dragState.armed = false;
      dragState.sawMove = false;
      console.error("Failed to start live preview dragging:", error);
    });
  };

  const handleDragGripPointerEnd = () => {
    const dragState = dragGripStateRef.current;
    if (!dragState.sawMove) {
      dragState.armed = false;
    }
  };

  const handleResizeHandlePointerDown =
    (direction: PreviewResizeDirection) => (event: PointerEvent<HTMLDivElement>) => {
      if (event.button !== 0) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();

      const resizeState = resizeStateRef.current;
      resizeState.armed = true;
      resizeState.sawResize = false;
      if (resizeState.armTimer !== null) {
        window.clearTimeout(resizeState.armTimer);
      }
      if (resizeState.saveTimer !== null) {
        window.clearTimeout(resizeState.saveTimer);
        resizeState.saveTimer = null;
      }

      resizeState.armTimer = window.setTimeout(() => {
        if (!resizeState.sawResize) {
          resizeState.armed = false;
        }
        resizeState.armTimer = null;
      }, 1200);

      void windowRef.startResizeDragging(direction).catch((error) => {
        if (resizeState.armTimer !== null) {
          window.clearTimeout(resizeState.armTimer);
          resizeState.armTimer = null;
        }
        resizeState.armed = false;
        resizeState.sawResize = false;
        console.error("Failed to start live preview resizing:", error);
      });
    };

  const handleResizeHandlePointerEnd = () => {
    const resizeState = resizeStateRef.current;
    if (resizeState.armTimer !== null) {
      window.clearTimeout(resizeState.armTimer);
      resizeState.armTimer = null;
    }
    if (!resizeState.sawResize) {
      resizeState.armed = false;
    }
  };

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        appearance.ctrlBackspaceDeleteLastWord &&
        canDelete &&
        event.key === "Backspace" &&
        event.ctrlKey &&
        !event.altKey &&
        !event.metaKey
      ) {
        event.preventDefault();
        event.stopPropagation();
        void invokePreviewAction("preview_delete_last_word_action");
        return;
      }

      // Plain Backspace → delete last character (no modifier keys)
      if (
        appearance.backspaceDeleteLastChar &&
        canDelete &&
        event.key === "Backspace" &&
        !event.ctrlKey &&
        !event.shiftKey &&
        !event.altKey &&
        !event.metaKey
      ) {
        event.preventDefault();
        event.stopPropagation();
        void invokePreviewAction("preview_delete_last_char_action");
        return;
      }

      const currentHotkey = buildPreviewHotkeyFromKeyboardEvent(event, osType);
      if (!currentHotkey) {
        return;
      }

      const triggerIfMatches = (
        configuredHotkey: string,
        canRun: boolean,
        command: string,
      ): boolean => {
        const normalizedConfigured = normalizePreviewHotkeyString(configuredHotkey);
        if (!normalizedConfigured || !canRun) {
          return false;
        }
        if (normalizedConfigured !== currentHotkey) {
          return false;
        }
        event.preventDefault();
        event.stopPropagation();
        void invokePreviewAction(command);
        return true;
      };

      if (
        triggerIfMatches(
          appearance.closeHotkey,
          true,
          "preview_close_action",
        )
      ) {
        return;
      }
      if (
        triggerIfMatches(
          appearance.insertHotkey,
          canInsert,
          "preview_insert_action",
        )
      ) {
        return;
      }
      if (
        triggerIfMatches(
          appearance.processHotkey,
          canProcess,
          "preview_llm_process_action",
        )
      ) {
        return;
      }
      if (
        triggerIfMatches(
          appearance.flushHotkey,
          canFlush,
          "preview_flush_action",
        )
      ) {
        return;
      }
      if (
        triggerIfMatches(
          appearance.deleteUntilDotOrCommaHotkey,
          canDelete,
          "preview_delete_until_dot_or_comma_action",
        )
      ) {
        return;
      }
      if (
        triggerIfMatches(
          appearance.deleteUntilDotHotkey,
          canDelete,
          "preview_delete_until_dot_action",
        )
      ) {
        return;
      }
      triggerIfMatches(appearance.clearHotkey, canClear, "preview_clear_action");
    };

    window.addEventListener("keydown", handleKeyDown, true);
    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [
    appearance.clearHotkey,
    appearance.closeHotkey,
    appearance.ctrlBackspaceDeleteLastWord,
    appearance.backspaceDeleteLastChar,
    appearance.deleteUntilDotHotkey,
    appearance.deleteUntilDotOrCommaHotkey,
    appearance.flushHotkey,
    appearance.insertHotkey,
    appearance.processHotkey,
    canClear,
    canDelete,
    canFlush,
    canInsert,
    canProcess,
    invokePreviewAction,
    osType,
  ]);

  return (
    <div className="soniox-live-preview-root" style={rootStyle}>
      {RESIZE_HANDLES.map(({ direction, className }) => (
        <div
          key={direction}
          aria-hidden="true"
          className={className}
          onPointerDown={handleResizeHandlePointerDown(direction)}
          onPointerUp={handleResizeHandlePointerEnd}
          onPointerCancel={handleResizeHandlePointerEnd}
        />
      ))}
      <button
        type="button"
        className="soniox-live-preview-close"
        onClick={handleClose}
        title={labelWithHotkey("Close", closeHotkeyLabel)}
      >
        {labelWithHotkey("X", closeHotkeyLabel)}
      </button>
      {appearance.showDragGrip && (
        <div className="soniox-live-preview-grip-row">
          <button
            type="button"
            className="soniox-live-preview-grip"
            aria-label="Drag to move window"
            title="Drag to move window"
            onPointerDown={handleDragGripPointerDown}
            onPointerUp={handleDragGripPointerEnd}
            onPointerCancel={handleDragGripPointerEnd}
          >
            {Array.from({ length: 6 }).map((_, index) => (
              <span key={index} className="soniox-live-preview-grip-dot" />
            ))}
          </button>
        </div>
      )}
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
          {appearance.showClearButton && (
            <button
              type="button"
              className="soniox-live-preview-action-button"
              onClick={handleClear}
              disabled={!canClear}
            >
              {labelWithHotkey("Clear all", clearHotkeyLabel)}
            </button>
          )}
          {appearance.showDeleteUntilDotOrCommaButton && (
            <button
              type="button"
              className="soniox-live-preview-action-button"
              onClick={handleDeleteUntilDotOrComma}
              disabled={!canDelete}
            >
              {labelWithHotkey("Delete to . / ,", deleteUntilDotOrCommaHotkeyLabel)}
            </button>
          )}
          {appearance.showDeleteUntilDotButton && (
            <button
              type="button"
              className="soniox-live-preview-action-button"
              onClick={handleDeleteUntilDot}
              disabled={!canDelete}
            >
              {labelWithHotkey("Delete to .", deleteUntilDotHotkeyLabel)}
            </button>
          )}
          {appearance.showDeleteLastWordButton && (
            <button
              type="button"
              className="soniox-live-preview-action-button"
              onClick={handleDeleteLastWord}
              disabled={!canDelete}
            >
              {labelWithHotkey("Delete last word", deleteLastWordHotkeyLabel)}
            </button>
          )}
          {appearance.showFlushButton && workflowState.flushVisible && (
            <button
              type="button"
              className="soniox-live-preview-action-button"
              onClick={handleFlush}
              disabled={!canFlush}
            >
              {labelWithHotkey("Flush", flushHotkeyLabel)}
            </button>
          )}
          {appearance.showProcessButton && (
            <button
              type="button"
              className="soniox-live-preview-action-button"
              onClick={handleProcess}
              disabled={!canProcess}
            >
              {labelWithHotkey(
                workflowState.processingLlm ? "Processing..." : "Processing via LLM",
                processHotkeyLabel,
              )}
            </button>
          )}
          {appearance.showInsertButton && (
            <button
              type="button"
              className="soniox-live-preview-action-button soniox-live-preview-action-button-primary"
              onClick={handleInsert}
              disabled={!canInsert}
            >
              {labelWithHotkey("Insert", insertHotkeyLabel)}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
