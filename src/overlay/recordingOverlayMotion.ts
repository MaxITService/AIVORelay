import type { CSSProperties } from "react";

type RecordingOverlayMotionState =
  | "recording"
  | "sending"
  | "thinking"
  | "finalizing"
  | "transcribing"
  | "error"
  | "profile_switch"
  | "microphone_switch";

export interface RecordingOverlayMotionOptions {
  isVisible?: boolean;
  state: RecordingOverlayMotionState;
  levels: number[];
  audioReactiveScale: boolean;
  audioReactiveScaleMaxPercent: number;
  voiceSensitivityPercent: number;
  animationSoftnessPercent: number;
  opacityPercent: number;
  silenceFade: boolean;
  silenceOpacityPercent: number;
}

function clampUnit(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }
  return Math.max(0, Math.min(1, value));
}

function clampPercent(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) {
    return min;
  }
  return Math.max(min, Math.min(max, Math.round(value)));
}

function computeEnergy(levels: number[]): number {
  if (levels.length === 0) {
    return 0;
  }

  const normalized = levels.map(clampUnit);
  const average =
    normalized.reduce((total, value) => total + value, 0) / normalized.length;
  const peak = normalized.reduce((max, value) => Math.max(max, value), 0);
  return clampUnit((average * 0.7) + (peak * 0.3));
}

export function getRecordingOverlayMotionStyle(
  options: RecordingOverlayMotionOptions,
): CSSProperties {
  const {
    isVisible = true,
    state,
    levels,
    audioReactiveScale,
    audioReactiveScaleMaxPercent,
    voiceSensitivityPercent,
    animationSoftnessPercent,
    opacityPercent,
    silenceFade,
    silenceOpacityPercent,
  } = options;
  const softness = clampPercent(animationSoftnessPercent, 0, 100) / 100;
  const opacityDurationMs = Math.round(140 + (softness * 180));
  const transformDurationMs = Math.round(110 + (softness * 170));

  if (!isVisible) {
    return {
      opacity: 0,
      transform: "scale(1)",
      transformOrigin: "center center",
      transition: `opacity ${opacityDurationMs}ms ease-out, transform ${transformDurationMs}ms ease-out`,
    };
  }

  const isReactiveState = state === "recording";
  const energy = isReactiveState ? computeEnergy(levels) : 0;
  const clampedVoiceSensitivity = clampPercent(voiceSensitivityPercent, 0, 100);
  const sensitivityOffset = (clampedVoiceSensitivity - 50) / 50;
  const responsiveEnergy =
    sensitivityOffset >= 0
      ? clampUnit(energy * (1 + (sensitivityOffset * 1.6)))
      : (() => {
          const threshold = Math.abs(sensitivityOffset) * 0.22;
          if (energy <= threshold) {
            return 0;
          }
          return clampUnit((energy - threshold) / (1 - threshold));
        })();
  const easedEnergy = Math.pow(responsiveEnergy, 0.8);

  const maxScaleBoost =
    (clampPercent(audioReactiveScaleMaxPercent, 0, 24) / 100) *
    (1.08 - (softness * 0.26));
  const scale =
    audioReactiveScale && isReactiveState
      ? 1 + (easedEnergy * maxScaleBoost)
      : 1;

  const minOpacity =
    clampPercent(silenceOpacityPercent, 20, 100) / 100;
  const baseOpacity =
    clampPercent(opacityPercent, 20, 100) / 100;
  const opacity =
    silenceFade && isReactiveState
      ? baseOpacity * (minOpacity + ((1 - minOpacity) * (0.16 + (easedEnergy * 0.84))))
      : baseOpacity;

  return {
    opacity,
    transform: `scale(${scale})`,
    transformOrigin: "center center",
    transition: `opacity ${opacityDurationMs}ms ease-out, transform ${transformDurationMs}ms ease-out`,
  };
}
