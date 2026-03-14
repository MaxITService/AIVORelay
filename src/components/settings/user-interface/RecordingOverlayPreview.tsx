import React, { useEffect, useMemo, useState } from "react";
import {
  CancelIcon,
  MicrophoneIcon,
  TranscriptionIcon,
} from "../../icons";
import {
  getRecordingOverlayBarStyle,
  getRecordingOverlaySurfaceStyle,
  normalizeRecordingOverlayBarStyle,
  normalizeRecordingOverlayColor,
  recordingOverlayHexToRgba,
  type RecordingOverlayBarStyle,
  type RecordingOverlayTheme,
} from "../../../overlay/recordingOverlayAppearance";

type PreviewState = "recording" | "transcribing" | "error";

interface RecordingOverlayPreviewProps {
  theme: RecordingOverlayTheme;
  accentColor: string;
  showStatusIcon: boolean;
  barCount: number;
  barWidthPx: number;
  barStyle: RecordingOverlayBarStyle;
  showDragGrip: boolean;
  state: PreviewState;
}

function createLevels(count: number): number[] {
  const safeCount = Number.isFinite(count) ? Math.max(3, Math.round(count)) : 16;
  return Array.from({ length: Math.max(safeCount, 16) }, () => 0.18 + Math.random() * 0.82);
}

export const RecordingOverlayPreview: React.FC<RecordingOverlayPreviewProps> = ({
  theme,
  accentColor,
  showStatusIcon,
  barCount,
  barWidthPx,
  barStyle,
  showDragGrip,
  state,
}) => {
  const normalizedAccent = normalizeRecordingOverlayColor(accentColor);
  const normalizedBarStyle = normalizeRecordingOverlayBarStyle(barStyle);
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
    () => getRecordingOverlaySurfaceStyle(theme, normalizedAccent, barWidthPx),
    [barWidthPx, normalizedAccent, theme],
  );

  const effectiveBarCount = Math.max(3, Math.min(16, Math.round(barCount)));
  const effectiveBarWidth = Math.max(2, Math.min(12, Math.round(barWidthPx)));
  const barTrackWidth =
    effectiveBarCount * effectiveBarWidth + (effectiveBarCount - 1) * 3;
  const overlayWidth =
    state === "error"
      ? 340
      : Math.max(172, 60 + (showStatusIcon ? 28 : 0) + barTrackWidth);
  const overlayHeight = state === "error" ? 82 : 36;
  const gripColor = recordingOverlayHexToRgba(normalizedAccent, 0.34);
  const cancelHover = recordingOverlayHexToRgba(normalizedAccent, 0.22);

  return (
    <div className="flex items-center justify-center rounded-xl border border-[#3a3a3a] bg-[#171717] px-6 py-6">
      <div
        style={{
          ...surfaceStyle,
          width: `${overlayWidth}px`,
          minHeight: `${overlayHeight}px`,
          display: "grid",
          gridTemplateColumns: "auto minmax(0, 1fr) auto",
          alignItems: "center",
          gap: "8px",
          padding: "6px 8px",
          position: "relative",
          boxSizing: "border-box",
          ...(state === "error"
            ? {
                background: "rgba(26, 0, 0, 0.8)",
                border: "1px solid rgba(255, 107, 107, 0.27)",
              }
            : {}),
        }}
      >
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

        <div className="flex items-center">
          {showStatusIcon ? (
            state === "recording" ? (
              <MicrophoneIcon />
            ) : state === "error" ? (
              <span style={{ fontSize: "16px", lineHeight: 1 }}>❌</span>
            ) : (
              <TranscriptionIcon />
            )
          ) : null}
        </div>

        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: state === "error" ? "flex-start" : "center",
            minWidth: 0,
          }}
        >
          {state === "recording" && (
            <div
              style={{
                display: "flex",
                alignItems: "flex-end",
                justifyContent: "center",
                gap: "3px",
                height: "24px",
              }}
            >
              {levels.slice(0, effectiveBarCount).map((level, index) => (
                <div
                  key={index}
                  style={{
                    width: `${effectiveBarWidth}px`,
                    height: `${Math.min(20, 4 + Math.pow(level, 0.7) * 16)}px`,
                    minHeight: "4px",
                    transition: "height 60ms ease-out, opacity 120ms ease-out",
                    ...getRecordingOverlayBarStyle(
                      normalizedBarStyle,
                      normalizedAccent,
                      level,
                      index,
                    ),
                  }}
                />
              ))}
            </div>
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

        <div className="flex items-center justify-end">
          {state === "recording" && (
            <div
              style={{
                width: "24px",
                height: "24px",
                borderRadius: "999px",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                background: cancelHover,
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
