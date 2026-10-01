import React from "react";
import "./RecordingOverlayStatusIcon.css";
import {
  mixRecordingOverlayHexColors,
  normalizeRecordingOverlayColor,
  recordingOverlayHexToRgba,
  type RecordingOverlayStatusIconStyle,
} from "./recordingOverlayAppearance";

export type RecordingOverlayStatusIconTone =
  | "recording"
  | "busy"
  | "idle"
  | "error";

interface RecordingOverlayStatusIconProps {
  frame: Exclude<RecordingOverlayStatusIconStyle, "auto">;
  tone: RecordingOverlayStatusIconTone;
  accentColor: string;
  iconColor: string;
  children: React.ReactNode;
}

/**
 * The custom overlay's status icon in its frame. The dot frame replaces the
 * icon; the coin frame adds an edge layer so the flip shows thickness.
 */
export const RecordingOverlayStatusIcon: React.FC<RecordingOverlayStatusIconProps> = ({
  frame,
  tone,
  accentColor,
  iconColor,
  children,
}) => {
  const accent = normalizeRecordingOverlayColor(accentColor);
  const icon = normalizeRecordingOverlayColor(iconColor, "#faa2ca");
  const style = {
    "--status-icon-accent": accent,
    "--status-icon-accent-soft": recordingOverlayHexToRgba(accent, 0.2),
    "--status-icon-accent-strong": recordingOverlayHexToRgba(accent, 0.6),
    "--status-icon-halo": recordingOverlayHexToRgba(accent, 0.28),
    "--status-icon-accent-light": mixRecordingOverlayHexColors(accent, "#ffffff", 0.5),
    "--status-icon-accent-deep": mixRecordingOverlayHexColors(accent, "#000000", 0.62),
    "--status-icon-accent-night": mixRecordingOverlayHexColors(accent, "#000000", 0.84),
    "--status-icon-color": icon,
    "--status-icon-color-halo": recordingOverlayHexToRgba(icon, 0.4),
  } as React.CSSProperties;

  return (
    <span
      className={`rec-status-icon rec-status-icon--${frame} is-${tone}`}
      style={style}
    >
      {frame === "dot" ? (
        <span className="rec-status-icon__dot" />
      ) : frame === "coin" ? (
        <>
          <span className="rec-status-icon__coin-edge" />
          <span className="rec-status-icon__coin-face">
            <span className="rec-status-icon__glyph">{children}</span>
          </span>
        </>
      ) : (
        <span className="rec-status-icon__glyph">{children}</span>
      )}
    </span>
  );
};
