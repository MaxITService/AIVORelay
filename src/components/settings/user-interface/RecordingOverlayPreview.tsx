import React, { useEffect, useMemo, useState } from "react";
import {
  CancelIcon,
  MicrophoneIcon,
  TranscriptionIcon,
} from "../../icons";
import {
  normalizeRecordingOverlayAnimatedBorderMode,
  getRecordingOverlayErrorStateStyle,
  getRecordingOverlaySurfaceStyle,
  normalizeRecordingOverlayBackgroundMode,
  normalizeRecordingOverlayBarStyle,
  normalizeRecordingOverlayCenterpieceMode,
  normalizeRecordingOverlayColor,
  normalizeRecordingOverlayMaterialMode,
  recordingOverlayHexToRgba,
  type RecordingOverlayAnimatedBorderMode,
  type RecordingOverlayBarStyle,
  type RecordingOverlayBackgroundMode,
  type RecordingOverlayCenterpieceMode,
  type RecordingOverlayMaterialMode,
  type RecordingOverlayTheme,
} from "../../../overlay/recordingOverlayAppearance";
import { RecordingOverlayBars } from "../../../overlay/RecordingOverlayBars";
import { RecordingOverlayAnimatedBorder } from "../../../overlay/RecordingOverlayAnimatedBorder";
import { RecordingOverlayBackground } from "../../../overlay/RecordingOverlayBackground";
import { RecordingOverlayCenterpiece } from "../../../overlay/RecordingOverlayCenterpiece";
import { getRecordingOverlayMotionStyle } from "../../../overlay/recordingOverlayMotion";

type PreviewState = "recording" | "transcribing" | "error";

interface RecordingOverlayPreviewProps {
  theme: RecordingOverlayTheme;
  accentColor: string;
  materialMode: RecordingOverlayMaterialMode;
  showStatusIcon: boolean;
  backgroundMode: RecordingOverlayBackgroundMode;
  centerpieceMode: RecordingOverlayCenterpieceMode;
  animatedBorderMode: RecordingOverlayAnimatedBorderMode;
  barCount: number;
  barWidthPx: number;
  barStyle: RecordingOverlayBarStyle;
  showDragGrip: boolean;
  state: PreviewState;
  audioReactiveScale: boolean;
  audioReactiveScaleMaxPercent: number;
  animationSoftnessPercent: number;
  depthParallaxPercent: number;
  opacityPercent: number;
  silenceFade: boolean;
  silenceOpacityPercent: number;
}

function createLevels(count: number): number[] {
  const safeCount = Number.isFinite(count) ? Math.max(3, Math.round(count)) : 16;
  return Array.from({ length: Math.max(safeCount, 16) }, () => 0.18 + Math.random() * 0.82);
}

export const RecordingOverlayPreview: React.FC<RecordingOverlayPreviewProps> = ({
  theme,
  accentColor,
  materialMode,
  showStatusIcon,
  backgroundMode,
  centerpieceMode,
  animatedBorderMode,
  barCount,
  barWidthPx,
  barStyle,
  showDragGrip,
  state,
  audioReactiveScale,
  audioReactiveScaleMaxPercent,
  animationSoftnessPercent,
  depthParallaxPercent,
  opacityPercent,
  silenceFade,
  silenceOpacityPercent,
}) => {
  const normalizedAccent = normalizeRecordingOverlayColor(accentColor);
  const normalizedBackgroundMode =
    normalizeRecordingOverlayBackgroundMode(backgroundMode);
  const normalizedMaterialMode =
    normalizeRecordingOverlayMaterialMode(materialMode);
  const normalizedBarStyle = normalizeRecordingOverlayBarStyle(barStyle);
  const normalizedCenterpieceMode =
    normalizeRecordingOverlayCenterpieceMode(centerpieceMode);
  const normalizedAnimatedBorderMode =
    normalizeRecordingOverlayAnimatedBorderMode(animatedBorderMode);
  const normalizedOpacityPercent = Math.max(20, Math.min(100, Math.round(opacityPercent)));
  const [levels, setLevels] = useState<number[]>(() => createLevels(barCount));

  useEffect(() => {
    setLevels(createLevels(barCount));
  }, [barCount]);

  useEffect(() => {
    if (state !== "recording") {
      return;
    }

    const intervalId = window.setInterval(() => {
      setLevels(createLevels(barCount));
    }, 150);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [barCount, state]);

  const surfaceStyle = useMemo(
    () =>
      getRecordingOverlaySurfaceStyle(
        theme,
        normalizedAccent,
        barWidthPx,
        opacityPercent,
        normalizedMaterialMode,
      ),
    [barWidthPx, normalizedAccent, normalizedMaterialMode, opacityPercent, theme],
  );

  const effectiveBarCount = Math.max(3, Math.min(16, Math.round(barCount)));
  const effectiveBarWidth = Math.max(2, Math.min(12, Math.round(barWidthPx)));
  const laneWidth =
    normalizedBarStyle === "vinyl"
      ? Math.max(effectiveBarWidth + 6, 10)
      : normalizedBarStyle === "constellation" ||
          normalizedBarStyle === "fireflies" ||
          normalizedBarStyle === "helix" ||
          normalizedBarStyle === "petals" ||
          normalizedBarStyle === "pulse_rings"
        ? Math.max(effectiveBarWidth + 8, 14)
      : normalizedBarStyle === "orbit" ||
          normalizedBarStyle === "tuner" ||
          normalizedBarStyle === "morse"
        ? Math.max(effectiveBarWidth + 2, 8)
        : effectiveBarWidth;
  const barTrackWidth =
    effectiveBarCount * laneWidth + (effectiveBarCount - 1) * 3;
  const overlayWidth =
    state === "error"
      ? 340
      : Math.max(172, 60 + (showStatusIcon ? 28 : 0) + barTrackWidth);
  const overlayHeight = state === "error" ? 82 : 36;
  const gripColor = recordingOverlayHexToRgba(normalizedAccent, 0.34);
  const cancelHover = recordingOverlayHexToRgba(normalizedAccent, 0.22);
  const stageGlow = recordingOverlayHexToRgba(normalizedAccent, 0.16);
  const stageGlowStrong = recordingOverlayHexToRgba(normalizedAccent, 0.22);
  const iconShellBorder = recordingOverlayHexToRgba(normalizedAccent, 0.18);
  const iconHalo = recordingOverlayHexToRgba(normalizedAccent, 0.28);
  const motionStyle = useMemo(
    () =>
      getRecordingOverlayMotionStyle({
        state: state === "recording" ? "recording" : state,
        levels: levels.slice(0, effectiveBarCount),
        audioReactiveScale,
        audioReactiveScaleMaxPercent,
        animationSoftnessPercent,
        opacityPercent,
        silenceFade,
        silenceOpacityPercent,
      }),
    [
      audioReactiveScale,
      audioReactiveScaleMaxPercent,
      animationSoftnessPercent,
      effectiveBarCount,
      levels,
      opacityPercent,
      silenceFade,
      silenceOpacityPercent,
      state,
    ],
  );
  const errorStyle = useMemo(
    () => getRecordingOverlayErrorStateStyle(normalizedOpacityPercent),
    [normalizedOpacityPercent],
  );

  return (
    <div
      className="relative flex items-center justify-center overflow-hidden rounded-[22px] border border-[#2f2f2f] px-6 py-6"
      style={{
        background: `
          radial-gradient(circle at 18% 22%, ${stageGlowStrong} 0%, rgba(0,0,0,0) 34%),
          radial-gradient(circle at 82% 18%, rgba(255,255,255,0.05) 0%, rgba(0,0,0,0) 26%),
          linear-gradient(180deg, #202020 0%, #121212 100%)
        `,
        boxShadow: `inset 0 1px 0 rgba(255,255,255,0.05), 0 18px 40px rgba(0,0,0,0.28), 0 0 0 1px ${recordingOverlayHexToRgba(normalizedAccent, 0.08)}`,
      }}
    >
      <div
        style={{
          position: "absolute",
          inset: 0,
          backgroundImage:
            "linear-gradient(rgba(255,255,255,0.035) 1px, transparent 1px), linear-gradient(90deg, rgba(255,255,255,0.035) 1px, transparent 1px)",
          backgroundSize: "22px 22px",
          maskImage: "linear-gradient(180deg, rgba(0,0,0,0.55), rgba(0,0,0,0.12))",
          opacity: 0.26,
          pointerEvents: "none",
        }}
      />
      <div
        style={{
          position: "absolute",
          width: "62%",
          height: "62%",
          borderRadius: "999px",
          background: `radial-gradient(circle, ${stageGlow} 0%, rgba(0,0,0,0) 72%)`,
          filter: "blur(18px)",
          opacity: 0.85,
          transform: "translateY(8px)",
          pointerEvents: "none",
        }}
      />
      <div
        style={{
          position: "absolute",
          inset: 0,
          backgroundImage:
            "radial-gradient(rgba(255,255,255,0.03) 0.6px, transparent 0.7px), radial-gradient(rgba(255,255,255,0.018) 0.6px, transparent 0.7px)",
          backgroundSize: "13px 13px, 17px 17px",
          backgroundPosition: "0 0, 7px 5px",
          opacity: 0.24,
          mixBlendMode: "screen",
          pointerEvents: "none",
        }}
      />
      <div
        style={{
          ...surfaceStyle,
          ...motionStyle,
          width: `${overlayWidth}px`,
          minHeight: `${overlayHeight}px`,
          display: "grid",
          gridTemplateColumns: "auto minmax(0, 1fr) auto",
          alignItems: "center",
          gap: "8px",
          padding: "6px 8px",
          position: "relative",
          boxSizing: "border-box",
          ...(state === "error" ? errorStyle : {}),
        }}
      >
        <div
          style={{
            position: "absolute",
            inset: 0,
            borderRadius: "inherit",
            background:
              state === "error"
                ? "linear-gradient(180deg, rgba(255,132,132,0.1) 0%, rgba(255,72,72,0.02) 100%)"
                : "none",
            pointerEvents: "none",
            zIndex: 0,
          }}
        />
        <RecordingOverlayBackground
          mode={normalizedBackgroundMode}
          accentColor={normalizedAccent}
          levels={levels.slice(0, effectiveBarCount)}
          animationSoftnessPercent={animationSoftnessPercent}
          depthParallaxPercent={depthParallaxPercent}
        />
        <RecordingOverlayCenterpiece
          mode={normalizedCenterpieceMode}
          accentColor={normalizedAccent}
          levels={levels.slice(0, effectiveBarCount)}
          animationSoftnessPercent={animationSoftnessPercent}
          depthParallaxPercent={depthParallaxPercent}
        />
        <RecordingOverlayAnimatedBorder
          mode={normalizedAnimatedBorderMode}
          accentColor={normalizedAccent}
          levels={levels.slice(0, effectiveBarCount)}
          animationSoftnessPercent={animationSoftnessPercent}
          depthParallaxPercent={depthParallaxPercent}
        />

        {showDragGrip && (
          <div
            style={{
              position: "absolute",
              top: "4px",
              left: 0,
              right: 0,
              display: "flex",
              justifyContent: "center",
              pointerEvents: "none",
            }}
          >
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "repeat(3, 4px)",
                gap: "2px 4px",
                width: "28px",
                height: "10px",
                alignContent: "center",
                justifyContent: "center",
              }}
            >
              {Array.from({ length: 6 }).map((_, index) => (
                <span
                  key={index}
                  style={{
                    width: "2px",
                    height: "2px",
                    borderRadius: "999px",
                    background: gripColor,
                  }}
                />
              ))}
            </div>
          </div>
        )}

        <div className="flex items-center" style={{ position: "relative", zIndex: 1 }}>
          {showStatusIcon ? (
            <div
              style={{
                position: "relative",
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                width: "24px",
                height: "24px",
                borderRadius: "999px",
                border: `1px solid ${state === "error" ? "rgba(255,123,123,0.18)" : iconShellBorder}`,
                background:
                  state === "error"
                    ? "linear-gradient(180deg, rgba(255,123,123,0.14) 0%, rgba(255,70,70,0.04) 100%)"
                    : "linear-gradient(180deg, rgba(255,255,255,0.08) 0%, rgba(255,255,255,0.02) 100%), rgba(255,255,255,0.03)",
                boxShadow:
                  "inset 0 1px 0 rgba(255,255,255,0.08), 0 0 0 1px rgba(0,0,0,0.06)",
              }}
            >
              <div
                style={{
                  position: "absolute",
                  inset: "-6px",
                  borderRadius: "999px",
                  background:
                    state === "error"
                      ? "radial-gradient(circle, rgba(255,107,107,0.26) 0%, rgba(0,0,0,0) 68%)"
                      : `radial-gradient(circle, ${iconHalo} 0%, rgba(0,0,0,0) 68%)`,
                  opacity: state === "recording" ? 0.76 : state === "error" ? 0.8 : 0.5,
                }}
              />
              <div style={{ position: "relative", zIndex: 1 }}>
                {state === "recording" ? (
                  <MicrophoneIcon />
                ) : state === "error" ? (
                  <span style={{ fontSize: "16px", lineHeight: 1 }}>❌</span>
                ) : (
                  <TranscriptionIcon />
                )}
              </div>
            </div>
          ) : null}
        </div>

        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: state === "error" ? "flex-start" : "center",
            minWidth: 0,
            position: "relative",
            zIndex: 1,
          }}
        >
          {state === "recording" && (
            <RecordingOverlayBars
              levels={levels}
              barCount={effectiveBarCount}
              barWidthPx={effectiveBarWidth}
              accentColor={normalizedAccent}
              barStyle={normalizedBarStyle}
              animationSoftnessPercent={animationSoftnessPercent}
            />
          )}
          {state === "transcribing" && (
            <div className="text-xs text-white">Transcribing...</div>
          )}
          {state === "error" && (
            <div className="min-w-0">
              <div className="text-xs font-semibold text-[#ffb3b3]">
                Transcription failed
              </div>
              <div className="text-[10px] leading-[1.15] text-[#ffd5d5]">
                Check your provider settings and try again.
              </div>
            </div>
          )}
        </div>

        <div className="flex items-center justify-end" style={{ position: "relative", zIndex: 1 }}>
          {state === "recording" && (
            <div
              style={{
                width: "24px",
                height: "24px",
                borderRadius: "999px",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                border: "1px solid rgba(255,255,255,0.08)",
                background:
                  `linear-gradient(180deg, rgba(255,255,255,0.08) 0%, rgba(255,255,255,0.02) 100%), ${cancelHover}`,
                boxShadow:
                  `inset 0 1px 0 rgba(255,255,255,0.08), 0 6px 12px rgba(0,0,0,0.16), 0 0 12px ${recordingOverlayHexToRgba(normalizedAccent, 0.12)}`,
              }}
            >
              <CancelIcon />
            </div>
          )}
          {state === "error" && (
            <span
              style={{
                color: "#ffd5d5",
                fontSize: "10px",
                fontFamily: '"Cascadia Mono", "Consolas", monospace',
                fontWeight: 600,
                letterSpacing: "0.02em",
                padding: "1px 5px",
                borderRadius: "999px",
                border: "1px solid rgba(255, 107, 107, 0.4)",
                background: "rgba(255, 107, 107, 0.1)",
              }}
            >
              NET
            </span>
          )}
        </div>
      </div>
    </div>
  );
};
