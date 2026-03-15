import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import React, { useEffect, useRef, useState } from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import {
  MicrophoneIcon,
  TranscriptionIcon,
  CancelIcon,
  SendingIcon,
  ThinkingIcon,
} from "../components/icons";
import "./RecordingOverlay.css";
import { commands, type RecordingOverlayAppearancePayload } from "@/bindings";
import { syncLanguageFromSettings } from "@/i18n";
import {
  normalizeRecordingOverlayAnimatedBorderMode,
  normalizeRecordingOverlayBackgroundMode,
  getRecordingOverlayErrorStateStyle,
  getRecordingOverlaySurfaceStyle,
  normalizeRecordingOverlayBarStyle,
  normalizeRecordingOverlayCenterpieceMode,
  normalizeRecordingOverlayColor,
  normalizeRecordingOverlayMaterialMode,
  type RecordingOverlayAnimatedBorderMode,
  type RecordingOverlayBarStyle,
  type RecordingOverlayBackgroundMode,
  type RecordingOverlayCenterpieceMode,
  type RecordingOverlayMaterialMode,
  type RecordingOverlayTheme,
} from "./recordingOverlayAppearance";
import { RecordingOverlayAnimatedBorder } from "./RecordingOverlayAnimatedBorder";
import { RecordingOverlayBars } from "./RecordingOverlayBars";
import { RecordingOverlayBackground } from "./RecordingOverlayBackground";
import { RecordingOverlayCenterpiece } from "./RecordingOverlayCenterpiece";
import { getRecordingOverlayMotionStyle } from "./recordingOverlayMotion";
import {
  ExtendedOverlayState,
  fallbackCodeFromCategory,
  isExtendedPayload,
} from "./plus_overlay_states";
import type {
  OverlayErrorCategory,
  OverlayErrorEnvelope,
  OverlayErrorPhase,
} from "./plus_overlay_states";

const windowRef = getCurrentWindow();

const COMPACT_ERROR_CODE_MAP: Record<string, string> = {
  E_AUTH: "AUTH",
  E_BADREQ: "BAD_REQ",
  E_BILL: "BILLING",
  E_RATE: "RATE",
  E_TIMEOUT: "TIMEOUT",
  E_NET: "NET",
  E_SERVER: "SERVER",
  E_PARSE: "PARSE",
  E_EXT: "EXT",
  E_MIC: "MIC",
  E_UNKNOWN: "UNKNOWN",
};

function compactOverlayErrorCode(rawCode: string): string {
  const normalized = rawCode.toUpperCase().replace(/\s+/g, " ").trim();
  const statusMatch = normalized.match(/\b([1-5]\d{2})\b/);
  if (statusMatch) {
    return statusMatch[1];
  }

  const lastToken = normalized.split(" ").pop() ?? normalized;
  if (COMPACT_ERROR_CODE_MAP[lastToken]) {
    return COMPACT_ERROR_CODE_MAP[lastToken];
  }
  if (lastToken.length <= 8) {
    return lastToken;
  }
  return lastToken.slice(0, 8);
}

type OverlayErrorCopy = {
  title: string;
  hint: string;
};

const DEFAULT_OVERLAY_APPEARANCE: RecordingOverlayAppearancePayload = {
  theme: "classic",
  background_mode: "none",
  material_mode: "liquid_glass",
  centerpiece_mode: "none",
  animated_border_mode: "none",
  accent_color: "#ff4d8d",
  show_status_icon: true,
  bar_count: 9,
  bar_width_px: 6,
  bar_style: "solid",
  show_drag_grip: false,
  audio_reactive_scale: false,
  audio_reactive_scale_max_percent: 12,
  animation_softness_percent: 55,
  depth_parallax_percent: 40,
  opacity_percent: 100,
  silence_fade: false,
  silence_opacity_percent: 58,
  frame_width_px: 172,
  frame_height_px: 36,
};

function getOverlayErrorCopy(
  t: TFunction,
  category?: OverlayErrorCategory,
  envelope?: OverlayErrorEnvelope,
): OverlayErrorCopy {
  const phase: OverlayErrorPhase = envelope?.phase ?? "unknown";

  switch (category) {
    case "Auth":
      return {
        title: t("overlay.errors.auth.title", "Check API key"),
        hint: t(
          "overlay.errors.auth.hint",
          "Open Settings and verify the API key for this provider.",
        ),
      };
    case "RateLimited":
      return {
        title: t("overlay.errors.rateLimited.title", "Rate limit reached"),
        hint: t(
          "overlay.errors.rateLimited.hint",
          "Wait a moment and try again.",
        ),
      };
    case "Billing":
      return {
        title: t("overlay.errors.billing.title", "Billing required"),
        hint: t(
          "overlay.errors.billing.hint",
          "Check provider balance, quota, or subscription.",
        ),
      };
    case "BadRequest":
      return {
        title: t("overlay.errors.badRequest.title", "Request rejected"),
        hint: t(
          "overlay.errors.badRequest.hint",
          "Check model, server URL, or provider settings.",
        ),
      };
    case "TlsCertificate":
      return {
        title: t("overlay.errors.tlsCertificate.title", "Certificate error"),
        hint: t(
          "overlay.errors.tlsCertificate.hint",
          "Check the HTTPS certificate or use a trusted server URL.",
        ),
      };
    case "TlsHandshake":
      return {
        title:
          phase === "connect"
            ? t("overlay.errors.tlsHandshake.connectTitle", "Secure connection failed")
            : t("overlay.errors.tlsHandshake.title", "Connection lost"),
        hint:
          phase === "connect"
            ? t(
                "overlay.errors.tlsHandshake.connectHint",
                "Check server URL, proxy, VPN, or TLS settings.",
              )
            : t(
                "overlay.errors.tlsHandshake.hint",
                "The live session stopped. Check internet and start again.",
              ),
      };
    case "Timeout":
      return {
        title:
          phase === "connect"
            ? t("overlay.errors.timeout.connectTitle", "Server took too long")
            : t("overlay.errors.timeout.title", "Request timed out"),
        hint:
          phase === "connect"
            ? t(
                "overlay.errors.timeout.connectHint",
                "Check internet or server URL and try again.",
              )
            : t(
                "overlay.errors.timeout.hint",
                "Try again when the connection is stable.",
              ),
      };
    case "NetworkError":
      return {
        title:
          phase === "connect"
            ? t("overlay.errors.network.connectTitle", "Can't reach server")
            : t("overlay.errors.network.title", "Connection lost"),
        hint:
          phase === "stream" || phase === "finalize"
            ? t(
                "overlay.errors.network.streamHint",
                "The live session stopped. Check internet and start again.",
              )
            : t(
                "overlay.errors.network.hint",
                "Check internet, VPN, or server URL and try again.",
              ),
      };
    case "ServerError":
      return {
        title: t("overlay.errors.server.title", "Server error"),
        hint: t(
          "overlay.errors.server.hint",
          "The provider is unavailable right now. Try again soon.",
        ),
      };
    case "ParseError":
      return {
        title: t("overlay.errors.parse.title", "Bad server response"),
        hint: t(
          "overlay.errors.parse.hint",
          "Try again or check provider compatibility settings.",
        ),
      };
    case "ExtensionOffline":
      return {
        title: t("overlay.errors.extensionOffline.title", "Extension offline"),
        hint: t(
          "overlay.errors.extensionOffline.hint",
          "Reconnect the browser extension and try again.",
        ),
      };
    case "MicrophoneUnavailable":
      return {
        title: t("overlay.errors.microphone.title", "Microphone unavailable"),
        hint: t(
          "overlay.errors.microphone.hint",
          "Check mic access, selected device, or other apps using it.",
        ),
      };
    case "Unknown":
    default:
      return {
        title: t("overlay.errors.unknown.title", "Transcription failed"),
        hint: t(
          "overlay.errors.unknown.hint",
          "Try again. If it keeps happening, check the logs.",
        ),
      };
  }
}

const RecordingOverlay: React.FC = () => {
  const { t } = useTranslation();
  const [isVisible, setIsVisible] = useState(false);
  const [state, setState] = useState<ExtendedOverlayState>("recording");
  const [transientMessage, setTransientMessage] = useState<string>("");
  const [decapIndicatorEligible, setDecapIndicatorEligible] = useState(false);
  const [decapIndicatorArmed, setDecapIndicatorArmed] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [errorHint, setErrorHint] = useState<string | null>(null);
  const [errorCode, setErrorCode] = useState<string | null>(null);
  const [errorTechnical, setErrorTechnical] = useState<string | null>(null);
  const [levels, setLevels] = useState<number[]>(Array(20).fill(0));
  const [appearance, setAppearance] = useState<RecordingOverlayAppearancePayload>(
    DEFAULT_OVERLAY_APPEARANCE,
  );
  const smoothedLevelsRef = useRef<number[]>(Array(20).fill(0));
  const dragGripStateRef = useRef<{
    armed: boolean;
    sawMove: boolean;
    saveTimer: number | null;
  }>({
    armed: false,
    sawMove: false,
    saveTimer: null,
  });

  useEffect(() => {
    let active = true;
    let unlistenMoved: (() => void) | null = null;
    const unlistenFns: Array<() => void> = [];

    const applyAppearance = (raw: unknown) => {
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

      const data = payload as Partial<RecordingOverlayAppearancePayload> & {
        showDragGrip?: boolean;
      };
      const theme =
        data.theme === "minimal" || data.theme === "glass" ? data.theme : "classic";

      setAppearance({
        theme,
        background_mode: normalizeRecordingOverlayBackgroundMode(
          data.background_mode,
        ),
        material_mode: normalizeRecordingOverlayMaterialMode(data.material_mode),
        centerpiece_mode: normalizeRecordingOverlayCenterpieceMode(
          data.centerpiece_mode,
        ),
        animated_border_mode: normalizeRecordingOverlayAnimatedBorderMode(
          data.animated_border_mode,
        ),
        accent_color: normalizeRecordingOverlayColor(data.accent_color),
        show_status_icon:
          typeof data.show_status_icon === "boolean"
            ? data.show_status_icon
            : DEFAULT_OVERLAY_APPEARANCE.show_status_icon,
        bar_count:
          typeof data.bar_count === "number"
            ? Math.max(3, Math.min(16, Math.round(data.bar_count)))
            : DEFAULT_OVERLAY_APPEARANCE.bar_count,
        bar_width_px:
          typeof data.bar_width_px === "number"
            ? Math.max(2, Math.min(12, Math.round(data.bar_width_px)))
            : DEFAULT_OVERLAY_APPEARANCE.bar_width_px,
        bar_style: normalizeRecordingOverlayBarStyle(data.bar_style),
        show_drag_grip:
          typeof data.show_drag_grip === "boolean"
            ? data.show_drag_grip
            : typeof data.showDragGrip === "boolean"
              ? data.showDragGrip
              : DEFAULT_OVERLAY_APPEARANCE.show_drag_grip,
        audio_reactive_scale:
          typeof data.audio_reactive_scale === "boolean"
            ? data.audio_reactive_scale
            : DEFAULT_OVERLAY_APPEARANCE.audio_reactive_scale,
        audio_reactive_scale_max_percent:
          typeof data.audio_reactive_scale_max_percent === "number"
            ? Math.max(0, Math.min(24, Math.round(data.audio_reactive_scale_max_percent)))
            : DEFAULT_OVERLAY_APPEARANCE.audio_reactive_scale_max_percent,
        animation_softness_percent:
          typeof data.animation_softness_percent === "number"
            ? Math.max(0, Math.min(100, Math.round(data.animation_softness_percent)))
            : DEFAULT_OVERLAY_APPEARANCE.animation_softness_percent,
        depth_parallax_percent:
          typeof data.depth_parallax_percent === "number"
            ? Math.max(0, Math.min(100, Math.round(data.depth_parallax_percent)))
            : DEFAULT_OVERLAY_APPEARANCE.depth_parallax_percent,
        opacity_percent:
          typeof data.opacity_percent === "number"
            ? Math.max(20, Math.min(100, Math.round(data.opacity_percent)))
            : DEFAULT_OVERLAY_APPEARANCE.opacity_percent,
        silence_fade:
          typeof data.silence_fade === "boolean"
            ? data.silence_fade
            : DEFAULT_OVERLAY_APPEARANCE.silence_fade,
        silence_opacity_percent:
          typeof data.silence_opacity_percent === "number"
            ? Math.max(20, Math.min(100, Math.round(data.silence_opacity_percent)))
            : DEFAULT_OVERLAY_APPEARANCE.silence_opacity_percent,
        frame_width_px:
          typeof data.frame_width_px === "number"
            ? Math.max(0, Math.round(data.frame_width_px))
            : DEFAULT_OVERLAY_APPEARANCE.frame_width_px,
        frame_height_px:
          typeof data.frame_height_px === "number"
            ? Math.max(0, Math.round(data.frame_height_px))
            : DEFAULT_OVERLAY_APPEARANCE.frame_height_px,
      });
    };

    const refreshAppearance = async () => {
      try {
        applyAppearance(await commands.getRecordingOverlayAppearance());
      } catch {
        // Keep the overlay resilient if settings refresh fails.
      }
    };

    const setup = async () => {
      for (const eventName of [
        "recording-overlay-appearance-update",
        "recording_overlay_appearance_update",
      ]) {
        const unlisten = await listen<unknown>(eventName, (event) => {
          applyAppearance(event.payload);
        });
        unlistenFns.push(unlisten);
      }

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
              await invoke("remember_recording_overlay_window_position", {
                xPx: logicalX,
                yPx: logicalY,
              });
            } catch (error) {
              console.error("Failed to remember recording overlay position:", error);
            } finally {
              dragState.armed = false;
              dragState.sawMove = false;
              dragState.saveTimer = null;
            }
          }, 180);
        });
      } catch (error) {
        console.error("Failed to subscribe to recording overlay move events:", error);
      }
    };

    void refreshAppearance();
    void setup();

    return () => {
      active = false;
      for (const unlisten of unlistenFns) {
        unlisten();
      }
      if (unlistenMoved) {
        unlistenMoved();
      }
      const dragState = dragGripStateRef.current;
      if (dragState.saveTimer !== null) {
        window.clearTimeout(dragState.saveTimer);
        dragState.saveTimer = null;
      }
    };
  }, []);

  useEffect(() => {
    let cleanup: (() => void) | undefined;

    const setupEventListeners = async () => {
      // Listen for show-overlay event from Rust
      const unlistenShow = await listen("show-overlay", async (event) => {
        // Sync language from settings each time overlay is shown
        await syncLanguageFromSettings();

        const payload = event.payload;
        // Handle both extended payload objects and legacy string payloads
        if (isExtendedPayload(payload)) {
          setState(payload.state);
          setDecapIndicatorEligible(payload.decapitalize_eligible ?? false);
          setDecapIndicatorArmed(payload.decapitalize_armed ?? false);
          if (payload.state === "error") {
            const envelope = payload.error_envelope;
            const copy = getOverlayErrorCopy(
              t,
              payload.error_category,
              envelope,
            );
            const rawCode =
              envelope?.display_code ||
              fallbackCodeFromCategory(payload.error_category);
            setErrorMessage(copy.title);
            setErrorHint(copy.hint);
            setErrorCode(compactOverlayErrorCode(rawCode));
            setErrorTechnical(envelope?.technical_message || null);
          } else {
            setErrorMessage(null);
            setErrorHint(null);
            setErrorCode(null);
            setErrorTechnical(null);
          }
        } else {
          // Legacy string payload (e.g., "recording" or "transcribing")
          setState(payload as ExtendedOverlayState);
          setDecapIndicatorEligible(false);
          setDecapIndicatorArmed(false);
          setErrorMessage(null);
          setErrorHint(null);
          setErrorCode(null);
          setErrorTechnical(null);
        }
        setIsVisible(true);
      });

      const unlistenMessageOverlay = await listen<{
        state: "profile_switch" | "microphone_switch";
        message: string;
      }>("show-message-overlay", async (event) => {
        await syncLanguageFromSettings();

        setTransientMessage(event.payload.message);
        setState(event.payload.state);
        setDecapIndicatorEligible(false);
        setDecapIndicatorArmed(false);
        setErrorMessage(null);
        setErrorHint(null);
        setErrorCode(null);
        setErrorTechnical(null);
        setIsVisible(true);
      });

      // Listen for hide-overlay event from Rust
      const unlistenHide = await listen("hide-overlay", () => {
        setIsVisible(false);
        setDecapIndicatorEligible(false);
        setDecapIndicatorArmed(false);
      });

      // Listen for mic-level updates
      const unlistenLevel = await listen<number[]>("mic-level", (event) => {
        const newLevels = event.payload as number[];

        // Apply smoothing to reduce jitter
        const smoothed = smoothedLevelsRef.current.map((prev, i) => {
          const target = newLevels[i] || 0;
          return prev * 0.7 + target * 0.3; // Smooth transition
        });

        smoothedLevelsRef.current = smoothed;
        setLevels(smoothed);
      });

      cleanup = () => {
        unlistenShow();
        unlistenMessageOverlay();
        unlistenHide();
        unlistenLevel();
      };
    };

    void setupEventListeners();

    return () => {
      cleanup?.();
    };
  }, [t]);

  useEffect(() => {
    const shouldPoll =
      isVisible &&
      decapIndicatorEligible &&
      state !== "profile_switch" &&
      state !== "microphone_switch" &&
      state !== "error";
    if (!shouldPoll) {
      if (!decapIndicatorEligible || !isVisible) {
        setDecapIndicatorArmed(false);
      }
      return;
    }

    let cancelled = false;
    let inFlight = false;

    const refreshArmState = async () => {
      if (inFlight) return;
      inFlight = true;
      try {
        const payload = await invoke<{
          decapitalizeEligible: boolean;
          decapitalizeArmed: boolean;
        }>("get_text_replacement_decapitalize_overlay_state");

        if (cancelled) return;
        setDecapIndicatorEligible(payload.decapitalizeEligible);
        setDecapIndicatorArmed(payload.decapitalizeArmed);
      } catch {
        if (cancelled) return;
        setDecapIndicatorEligible(false);
        setDecapIndicatorArmed(false);
      } finally {
        inFlight = false;
      }
    };

    void refreshArmState();
    const intervalId = window.setInterval(() => {
      void refreshArmState();
    }, 250);

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [decapIndicatorEligible, isVisible, state]);

  const overlayTheme = appearance.theme as RecordingOverlayTheme;
  const backgroundMode =
    appearance.background_mode as RecordingOverlayBackgroundMode;
  const materialMode =
    appearance.material_mode as RecordingOverlayMaterialMode;
  const centerpieceMode =
    appearance.centerpiece_mode as RecordingOverlayCenterpieceMode;
  const animatedBorderMode =
    appearance.animated_border_mode as RecordingOverlayAnimatedBorderMode;
  const barStyle = appearance.bar_style as RecordingOverlayBarStyle;
  const showDragGrip = appearance.show_drag_grip;
  const showStatusIcon = appearance.show_status_icon;
  const visibleLevels = levels.slice(0, appearance.bar_count);
  const surfaceStyle = getRecordingOverlaySurfaceStyle(
    overlayTheme,
    appearance.accent_color,
    appearance.bar_width_px,
    appearance.opacity_percent,
    materialMode,
  );
  const motionStyle = getRecordingOverlayMotionStyle({
    isVisible,
    state:
      state === "recording" ||
      state === "sending" ||
      state === "thinking" ||
      state === "finalizing" ||
      state === "transcribing" ||
      state === "error" ||
      state === "profile_switch" ||
      state === "microphone_switch"
        ? state
        : "transcribing",
    levels: visibleLevels,
    audioReactiveScale: appearance.audio_reactive_scale,
    audioReactiveScaleMaxPercent: appearance.audio_reactive_scale_max_percent,
    animationSoftnessPercent: appearance.animation_softness_percent,
    opacityPercent: appearance.opacity_percent,
    silenceFade: appearance.silence_fade,
    silenceOpacityPercent: appearance.silence_opacity_percent,
  });
  const errorSurfaceStyle = getRecordingOverlayErrorStateStyle(
    appearance.opacity_percent,
  );

  const getIcon = () => {
    switch (state) {
      case "recording":
        return <MicrophoneIcon />;
      case "sending":
        return <SendingIcon />;
      case "thinking":
        return <ThinkingIcon />;
      case "finalizing":
        return <TranscriptionIcon />;
      case "error":
        return <span className="overlay-icon-emoji">❌</span>;
      case "profile_switch":
        return <TranscriptionIcon />;
      case "microphone_switch":
        return <MicrophoneIcon />;
      case "transcribing":
      default:
        return <TranscriptionIcon />;
      }
  };

  const iconStateClass =
    state === "error"
      ? "is-error"
      : state === "recording"
        ? "is-recording"
        : state === "sending" || state === "thinking" || state === "finalizing"
          ? "is-busy"
          : "is-idle";
  const overlayStateClass =
    state === "recording"
      ? "overlay-state-recording"
      : state === "sending" || state === "thinking" || state === "finalizing"
        ? "overlay-state-busy"
        : state === "error"
          ? "overlay-state-error"
          : "overlay-state-idle";

  const handleDragGripPointerDown = (event: React.PointerEvent<HTMLButtonElement>) => {
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
      console.error("Failed to start recording overlay dragging:", error);
    });
  };

  const handleDragGripPointerEnd = () => {
    const dragState = dragGripStateRef.current;
    if (!dragState.sawMove) {
      dragState.armed = false;
    }
  };

  return (
    <div
      className={`recording-overlay ${overlayStateClass} ${isVisible ? "fade-in" : ""} ${state === "error" ? "overlay-error" : ""} ${state === "microphone_switch" ? "overlay-microphone-switch" : ""}`}
      style={{
        ...surfaceStyle,
        ...motionStyle,
        ...(state === "error" ? errorSurfaceStyle : {}),
        width: `${appearance.frame_width_px}px`,
        minHeight: `${appearance.frame_height_px}px`,
      }}
    >
      <div className="recording-overlay-sheen" />
      <div className="recording-overlay-vignette" />
      <div className="recording-overlay-core-glow" />
      <div className="recording-overlay-grain" />

      <RecordingOverlayBackground
        mode={backgroundMode}
        accentColor={appearance.accent_color}
        levels={visibleLevels}
        animationSoftnessPercent={appearance.animation_softness_percent}
        depthParallaxPercent={appearance.depth_parallax_percent}
      />
      <RecordingOverlayCenterpiece
        mode={centerpieceMode}
        accentColor={appearance.accent_color}
        levels={visibleLevels}
        animationSoftnessPercent={appearance.animation_softness_percent}
        depthParallaxPercent={appearance.depth_parallax_percent}
      />
      <RecordingOverlayAnimatedBorder
        mode={animatedBorderMode}
        accentColor={appearance.accent_color}
        levels={visibleLevels}
        animationSoftnessPercent={appearance.animation_softness_percent}
        depthParallaxPercent={appearance.depth_parallax_percent}
      />

      {showDragGrip && (
        <div className="recording-overlay-grip-row">
          <button
            type="button"
            className="recording-overlay-grip"
            aria-label="Drag to move overlay"
            title="Drag to move overlay"
            onPointerDown={handleDragGripPointerDown}
            onPointerUp={handleDragGripPointerEnd}
            onPointerCancel={handleDragGripPointerEnd}
          >
            {Array.from({ length: 6 }).map((_, index) => (
              <span key={index} className="recording-overlay-grip-dot" />
            ))}
          </button>
        </div>
      )}
      {decapIndicatorEligible &&
        decapIndicatorArmed &&
        state !== "profile_switch" &&
        state !== "microphone_switch" &&
        state !== "error" && (
        <div className="overlay-decapitalize-indicator">
          {t("overlay.decapitalizationIndicator", "Decapitalization")}
        </div>
      )}

      <div className="overlay-left">
        {showStatusIcon ? (
          <div className={`overlay-icon-wrap ${iconStateClass}`}>
            {getIcon()}
          </div>
        ) : null}
      </div>

      <div className="overlay-middle">
        {state === "recording" && (
          <RecordingOverlayBars
            levels={visibleLevels}
            barCount={appearance.bar_count}
            barWidthPx={appearance.bar_width_px}
            accentColor={appearance.accent_color}
            barStyle={barStyle}
            animationSoftnessPercent={appearance.animation_softness_percent}
          />
        )}
        {state === "sending" && (
          <div className="sending-text">Sending...</div>
        )}
        {state === "thinking" && (
          <div className="thinking-text">Thinking...</div>
        )}
        {state === "finalizing" && (
          <div className="transcribing-text">{t("overlay.finalizing", "Finalizing...")}</div>
        )}
        {state === "transcribing" && (
          <div className="transcribing-text">{t("overlay.transcribing")}</div>
        )}
        {state === "error" && (
          <div className="error-copy">
            <span className="error-title">
              {errorMessage || t("overlay.errors.unknown.title", "Transcription failed")}
            </span>
            <span className="error-hint">
              {errorHint || t("overlay.errors.unknown.hint", "Try again and check the logs if needed.")}
            </span>
          </div>
        )}
        {state === "profile_switch" && (
          <div className="transcribing-text">{transientMessage}</div>
        )}
        {state === "microphone_switch" && (
          <div className="microphone-switch-copy">
            <span className="microphone-switch-label">
              {t("settings.sound.microphone.title", "Microphone")}
            </span>
            <span className="microphone-switch-name">{transientMessage}</span>
          </div>
        )}
      </div>

      <div className="overlay-right">
        {/* Show cancel button for: recording, sending, thinking, finalizing */}
        {(state === "recording" ||
          state === "sending" ||
          state === "thinking" ||
          state === "finalizing") && (
          <button
            type="button"
            className="cancel-button"
            onClick={() => {
              commands.cancelOperation();
            }}
          >
            <CancelIcon />
          </button>
        )}
        {state === "error" && (
          <span
            className="error-code-chip"
            title={errorTechnical || undefined}
          >
            {errorCode || "E_UNKNOWN"}
          </span>
        )}
      </div>
    </div>
  );
};

export default RecordingOverlay;
