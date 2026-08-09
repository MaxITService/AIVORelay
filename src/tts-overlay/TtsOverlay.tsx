import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { type as getOsType } from "@tauri-apps/plugin-os";
import {
  type ChangeEvent,
  type CSSProperties,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import type { OSType } from "../lib/utils/keyboard";
import {
  buildPreviewHotkeyFromKeyboardEvent,
  formatPreviewHotkeyForDisplay,
  normalizePreviewHotkeyString,
} from "../lib/utils/previewHotkeys";
import {
  DEFAULT_PLAYBACK_RATE,
  formatPlaybackRate,
  nextPlaybackRate,
  type PlaybackRate,
} from "../lib/utils/playbackRate";
import type { TtsPlaybackEffect } from "../lib/utils/ttsPlaybackEffects";
import {
  GaplessTtsPlayer,
  type GaplessPlaybackSnapshot,
} from "./GaplessTtsPlayer";

type TtsStatus =
  | "idle"
  | "loading"
  | "preprocessing"
  | "retrying"
  | "ready"
  | "playing"
  | "paused"
  | "stopped"
  | "completed"
  | "error";

type TtsChunk = {
  index: number;
  path: string;
  pauseAfterMs: number;
};

type TtsOverlayState = {
  operationId: string;
  status: TtsStatus;
  provider: string;
  model: string;
  voice: string;
  textPreview: string;
  chunks: TtsChunk[];
  currentChunk: number;
  totalChunks: number;
  retryAttempt: number;
  error: string | null;
  playPauseHotkey: string;
  playHistoryWhenOverlayClosed: boolean;
  stopHotkey: string;
  autoplay: boolean;
  autoHideEnabled: boolean;
  autoHideDelaySeconds: number;
  playbackPitch: number;
  playbackEffect: TtsPlaybackEffect;
};

type UnknownRecord = Record<string, unknown>;

const overlayWindow = getCurrentWindow();

const EMPTY_STATE: TtsOverlayState = {
  operationId: "",
  status: "idle",
  provider: "",
  model: "",
  voice: "",
  textPreview: "",
  chunks: [],
  currentChunk: 0,
  totalChunks: 0,
  retryAttempt: 0,
  error: null,
  playPauseHotkey: "",
  playHistoryWhenOverlayClosed: false,
  stopHotkey: "",
  autoplay: true,
  autoHideEnabled: true,
  autoHideDelaySeconds: 4,
  playbackPitch: 1,
  playbackEffect: "none",
};

const VALID_STATUSES = new Set<TtsStatus>([
  "idle",
  "loading",
  "preprocessing",
  "retrying",
  "ready",
  "playing",
  "paused",
  "stopped",
  "completed",
  "error",
]);

function asRecord(value: unknown): UnknownRecord | null {
  if (typeof value === "string") {
    try {
      return asRecord(JSON.parse(value));
    } catch {
      return null;
    }
  }
  return value && typeof value === "object" ? (value as UnknownRecord) : null;
}

function readString(
  data: UnknownRecord,
  camelName: string,
  snakeName: string,
  fallback = "",
): string {
  const value = data[camelName] ?? data[snakeName];
  return typeof value === "string" ? value : fallback;
}

function readNumber(
  data: UnknownRecord,
  camelName: string,
  snakeName: string,
  fallback = 0,
): number {
  const value = data[camelName] ?? data[snakeName];
  return typeof value === "number" && Number.isFinite(value)
    ? Math.max(0, Math.floor(value))
    : fallback;
}

function readBoolean(
  data: UnknownRecord,
  camelName: string,
  snakeName: string,
  fallback = false,
): boolean {
  const value = data[camelName] ?? data[snakeName];
  return typeof value === "boolean" ? value : fallback;
}

function readFiniteNumber(
  data: UnknownRecord,
  camelName: string,
  snakeName: string,
  fallback: number,
): number {
  const value = data[camelName] ?? data[snakeName];
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function safeErrorMessage(value: unknown): string | null {
  if (typeof value !== "string" || !value.trim()) {
    return null;
  }

  return value
    .trim()
    .replace(
      /(\b(?:api[-_ ]?key|authorization)\b\s*[:=]\s*)(?:bearer\s+)?[^\s,;]+/gi,
      "$1[redacted]",
    )
    .replace(/\bBearer\s+[A-Za-z0-9._~+/-]{8,}=*\b/gi, "Bearer [redacted]");
}

function normalizeState(raw: unknown): TtsOverlayState | null {
  const data = asRecord(raw);
  if (!data) {
    return null;
  }

  const rawStatus = readString(data, "status", "status").toLowerCase();
  const status = VALID_STATUSES.has(rawStatus as TtsStatus)
    ? (rawStatus as TtsStatus)
    : "idle";
  const rawChunks = data.chunks;
  const chunks = Array.isArray(rawChunks)
    ? rawChunks
        .map((rawChunk): TtsChunk | null => {
          const chunk = asRecord(rawChunk);
          if (!chunk) {
            return null;
          }
          const path = readString(chunk, "path", "path");
          const rawIndex = chunk.index;
          const index =
            typeof rawIndex === "number" &&
            Number.isFinite(rawIndex) &&
            rawIndex >= 0
              ? Math.floor(rawIndex)
              : null;
          return path && index !== null
            ? {
                index,
                path,
                pauseAfterMs: readNumber(
                  chunk,
                  "pauseAfterMs",
                  "pause_after_ms",
                ),
              }
            : null;
        })
        .filter((chunk): chunk is TtsChunk => chunk !== null)
        .sort((a, b) => a.index - b.index)
    : [];

  return {
    operationId: readString(data, "operationId", "operation_id"),
    status,
    provider: readString(data, "provider", "provider"),
    model: readString(data, "model", "model"),
    voice: readString(data, "voice", "voice"),
    textPreview: readString(data, "textPreview", "text_preview"),
    chunks,
    currentChunk: readNumber(data, "currentChunk", "current_chunk"),
    totalChunks: readNumber(data, "totalChunks", "total_chunks"),
    retryAttempt: readNumber(data, "retryAttempt", "retry_attempt"),
    error: safeErrorMessage(data.error),
    playPauseHotkey: normalizePreviewHotkeyString(
      readString(data, "playPauseHotkey", "play_pause_hotkey"),
    ),
    playHistoryWhenOverlayClosed: readBoolean(
      data,
      "playHistoryWhenOverlayClosed",
      "play_history_when_overlay_closed",
      false,
    ),
    stopHotkey: normalizePreviewHotkeyString(
      readString(data, "stopHotkey", "stop_hotkey"),
    ),
    autoplay:
      typeof (data.autoplay ?? data.auto_play) === "boolean"
        ? Boolean(data.autoplay ?? data.auto_play)
        : true,
    autoHideEnabled: readBoolean(
      data,
      "autoHideEnabled",
      "auto_hide_enabled",
      true,
    ),
    autoHideDelaySeconds: Math.min(
      300,
      Math.max(
        1,
        readNumber(
          data,
          "autoHideDelaySeconds",
          "auto_hide_delay_seconds",
          4,
        ),
      ),
    ),
    playbackPitch: Math.min(
      2,
      Math.max(
        0.5,
        readFiniteNumber(data, "playbackPitch", "playback_pitch", 1),
      ),
    ),
    playbackEffect: ["radio", "retro"].includes(
      readString(data, "playbackEffect", "playback_effect"),
    )
      ? (readString(
          data,
          "playbackEffect",
          "playback_effect",
        ) as TtsPlaybackEffect)
      : "none",
  };
}

function statusLabel(
  t: TFunction,
  state: TtsOverlayState,
  isPlaying: boolean,
  waitingForChunk: boolean,
): string {
  if (isPlaying) {
    return t("textToSpeech.overlayPlayer.playing");
  }
  if (state.status === "retrying") {
    return state.retryAttempt > 0
      ? t("textToSpeech.overlayPlayer.retryingAttempt", {
          attempt: state.retryAttempt,
        })
      : t("textToSpeech.overlayPlayer.retrying");
  }
  if (state.status === "loading") {
    return t("textToSpeech.overlayPlayer.preparing");
  }
  if (state.status === "preprocessing") {
    return t("textToSpeech.overlayPlayer.aiPreprocessing");
  }
  if (waitingForChunk) {
    return t("textToSpeech.overlayPlayer.waiting");
  }
  switch (state.status) {
    case "ready":
      return t("textToSpeech.overlayPlayer.ready");
    case "paused":
      return t("textToSpeech.overlayPlayer.paused");
    case "stopped":
      return t("textToSpeech.overlayPlayer.stopped");
    case "completed":
      return t("textToSpeech.overlayPlayer.completed");
    case "error":
      return t("textToSpeech.overlayPlayer.providerError");
    default:
      return t("textToSpeech.title");
  }
}

export default function TtsOverlay() {
  const { t } = useTranslation();
  const osKind = getOsType();
  const osType: OSType =
    osKind === "windows" || osKind === "macos" || osKind === "linux"
      ? osKind
      : "unknown";
  const [state, setState] = useState<TtsOverlayState>(EMPTY_STATE);
  const [isPlaying, setIsPlaying] = useState(false);
  const [activeChunkIndex, setActiveChunkIndex] = useState<number | null>(null);
  const [playbackDuration, setPlaybackDuration] = useState(0);
  const [playbackProgress, setPlaybackProgress] = useState(0);
  const [completedPlaybackOperationId, setCompletedPlaybackOperationId] =
    useState("");
  const [playbackError, setPlaybackError] = useState<string | null>(null);
  const [playbackRate, setPlaybackRate] = useState<PlaybackRate>(
    DEFAULT_PLAYBACK_RATE,
  );
  const playerRef = useRef<GaplessTtsPlayer | null>(null);
  const playbackRateRef = useRef<PlaybackRate>(DEFAULT_PLAYBACK_RATE);
  const stateRef = useRef(state);
  const desiredPlayingRef = useRef(false);
  const activeChunkIndexRef = useRef<number | null>(null);
  const operationIdRef = useRef("");
  const lastLoggedProviderErrorRef = useRef("");

  useEffect(() => {
    stateRef.current = state;
  }, [state]);

  const reportPlaybackState = useCallback(
    (
      status: "playing" | "paused" | "stopped" | "completed",
      chunk?: number,
    ) => {
      const operationId = operationIdRef.current;
      if (!operationId) {
        return;
      }
      void invoke("tts_overlay_playback_state", {
        operationId,
        status,
        currentChunk: chunk ?? activeChunkIndexRef.current,
      }).catch((error) => {
        console.error("Unable to report TTS overlay playback state:", error);
      });
    },
    [],
  );

  const getPlayer = useCallback(() => {
    const callbacks = {
      onSnapshot: (snapshot: GaplessPlaybackSnapshot) => {
        activeChunkIndexRef.current = snapshot.chunkIndex;
        setActiveChunkIndex(snapshot.chunkIndex);
        setIsPlaying(snapshot.isPlaying);
        setPlaybackDuration(snapshot.playbackDuration);
        setPlaybackProgress(snapshot.playbackProgress);
      },
      onChunkStart: (chunkIndex: number) => {
        activeChunkIndexRef.current = chunkIndex;
        setActiveChunkIndex(chunkIndex);
        setPlaybackError(null);
        reportPlaybackState("playing", chunkIndex);
      },
      onCompleted: (chunkIndex: number) => {
        desiredPlayingRef.current = false;
        setIsPlaying(false);
        setCompletedPlaybackOperationId(operationIdRef.current);
        reportPlaybackState("completed", chunkIndex);
      },
      onError: (error: unknown, chunkIndex: number) => {
        const message = t("textToSpeech.overlayPlayer.chunkPlaybackError");
        desiredPlayingRef.current = false;
        setIsPlaying(false);
        setPlaybackError(message);
        console.error(
          `TTS playback error at chunk ${chunkIndex + 1}:`,
          error,
        );
        reportPlaybackState("paused", chunkIndex);
      },
    };
    if (!playerRef.current) {
      playerRef.current = new GaplessTtsPlayer(callbacks);
    } else {
      playerRef.current.setCallbacks(callbacks);
    }
    return playerRef.current;
  }, [reportPlaybackState, t]);

  const resetAudio = useCallback(() => {
    playerRef.current?.stop();
    playerRef.current = null;
    activeChunkIndexRef.current = null;
    setActiveChunkIndex(null);
    setIsPlaying(false);
    setPlaybackDuration(0);
    setPlaybackProgress(0);
  }, []);

  const startAudio = useCallback(
    (player: GaplessTtsPlayer) => {
      void player.play().catch((error) => {
        if (playerRef.current !== player || !desiredPlayingRef.current) {
          return;
        }
        desiredPlayingRef.current = false;
        setIsPlaying(false);
        setPlaybackError(t("textToSpeech.overlayPlayer.playbackStartError"));
        console.error("Unable to start TTS playback:", error);
        reportPlaybackState("paused");
      });
    },
    [reportPlaybackState, t],
  );

  const applyState = useCallback(
    (raw: unknown) => {
      const next = normalizeState(raw);
      if (!next) {
        return;
      }

      const operationChanged =
        Boolean(next.operationId) &&
        next.operationId !== operationIdRef.current;
      if (operationChanged) {
        resetAudio();
        operationIdRef.current = next.operationId;
        desiredPlayingRef.current =
          next.autoplay &&
          next.status !== "paused" &&
          next.status !== "stopped" &&
          next.status !== "error";
        setPlaybackError(null);
        setCompletedPlaybackOperationId("");
        lastLoggedProviderErrorRef.current = "";
      } else if (!operationIdRef.current && next.operationId) {
        operationIdRef.current = next.operationId;
      }

      stateRef.current = next;
      setState(next);

      if (next.error) {
        const errorSignature = `${next.operationId}:${next.provider}:${next.error}`;
        if (lastLoggedProviderErrorRef.current !== errorSignature) {
          lastLoggedProviderErrorRef.current = errorSignature;
          console.error(
            `TTS provider error${next.provider ? ` (${next.provider})` : ""}: ${next.error}`,
          );
        }
      }

      if (next.status === "stopped" || next.status === "error") {
        desiredPlayingRef.current = false;
        resetAudio();
        return;
      }

      const player = getPlayer();
      player.configure(
        next.playbackPitch,
        next.playbackEffect,
        playbackRateRef.current,
      );
      player.setChunks(next.chunks, next.totalChunks);
      if (desiredPlayingRef.current && next.chunks.length > 0) {
        startAudio(player);
      }
    },
    [getPlayer, resetAudio, startAudio],
  );

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    let eventRevision = 0;

    const initialize = async () => {
      try {
        const dispose = await listen<unknown>("tts-overlay-state", (event) => {
          eventRevision += 1;
          if (active) {
            applyState(event.payload);
          }
        });
        if (!active) {
          dispose();
          return;
        }
        unlisten = dispose;
        const snapshotRevision = eventRevision;

        const initialState = await invoke<unknown>("get_tts_overlay_state");
        if (active && eventRevision === snapshotRevision) {
          applyState(initialState);
        }
      } catch (error) {
        if (!active) {
          return;
        }
        const message = t("textToSpeech.overlayPlayer.statusLoadError");
        setPlaybackError(message);
        console.error(message, error);
      }
    };

    void initialize();

    return () => {
      active = false;
      unlisten?.();
      resetAudio();
    };
  }, [applyState, resetAudio, t]);

  const togglePlayback = useCallback(() => {
    const player = getPlayer();
    if (isPlaying || desiredPlayingRef.current) {
      desiredPlayingRef.current = false;
      player.pause();
      reportPlaybackState("paused");
      return;
    }

    desiredPlayingRef.current = true;
    setPlaybackError(null);
    player.configure(
      stateRef.current.playbackPitch,
      stateRef.current.playbackEffect,
      playbackRateRef.current,
    );
    player.setChunks(
      stateRef.current.chunks,
      stateRef.current.totalChunks,
    );
    if (stateRef.current.chunks.length > 0) {
      startAudio(player);
    }
  }, [getPlayer, isPlaying, reportPlaybackState, startAudio]);

  const stopPlayback = useCallback(() => {
    const operationId = operationIdRef.current;
    desiredPlayingRef.current = false;
    resetAudio();
    reportPlaybackState("stopped");
    void invoke("cancel_tts_operation", {
      operationId: operationId || null,
    }).catch((error) => {
      const message = t("textToSpeech.overlayPlayer.cancelError");
      setPlaybackError(message);
      console.error(message, error);
    });
  }, [reportPlaybackState, resetAudio, t]);

  const closeOverlay = useCallback(() => {
    stopPlayback();
    void overlayWindow.hide().catch((error) => {
      console.error("Unable to hide the TTS overlay:", error);
    });
  }, [stopPlayback]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void overlayWindow.onCloseRequested((event) => {
      event.preventDefault();
      if (!disposed) {
        closeOverlay();
      }
    }).then((dispose) => {
      if (disposed) dispose();
      else unlisten = dispose;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [closeOverlay]);

  useEffect(() => {
    if (
      !state.autoHideEnabled ||
      state.status !== "completed" ||
      !state.operationId ||
      completedPlaybackOperationId !== state.operationId
    ) {
      return;
    }

    const completedOperationId = state.operationId;
    const timeout = window.setTimeout(() => {
      void overlayWindow
        .hide()
        .then(() => {
          if (
            stateRef.current.operationId === completedOperationId &&
            stateRef.current.status === "completed"
          ) {
            resetAudio();
          }
        })
        .catch((error) => {
          console.error("Unable to auto-hide the TTS overlay:", error);
        });
    }, state.autoHideDelaySeconds * 1_000);

    return () => window.clearTimeout(timeout);
  }, [
    completedPlaybackOperationId,
    resetAudio,
    state.autoHideDelaySeconds,
    state.autoHideEnabled,
    state.operationId,
    state.status,
  ]);

  const cyclePlaybackRate = useCallback(() => {
    const nextRate = nextPlaybackRate(playbackRateRef.current);
    playbackRateRef.current = nextRate;
    setPlaybackRate(nextRate);
    playerRef.current?.setPlaybackRate(nextRate);
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<string>("tts-overlay-control", (event) => {
      if (!disposed && event.payload === "play_pause") {
        togglePlayback();
      }
    }).then((dispose) => {
      if (disposed) dispose();
      else unlisten = dispose;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [togglePlayback]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!document.hasFocus() || event.repeat) {
        return;
      }
      const currentHotkey = buildPreviewHotkeyFromKeyboardEvent(event, osType);
      if (!currentHotkey) {
        return;
      }

      const matches = (configuredHotkey: string) =>
        Boolean(configuredHotkey) &&
        normalizePreviewHotkeyString(configuredHotkey) === currentHotkey;

      if (
        !state.playHistoryWhenOverlayClosed &&
        matches(state.playPauseHotkey)
      ) {
        event.preventDefault();
        event.stopPropagation();
        togglePlayback();
        return;
      }
      if (matches(state.stopHotkey)) {
        event.preventDefault();
        event.stopPropagation();
        stopPlayback();
      }
    };

    window.addEventListener("keydown", handleKeyDown, true);
    return () => window.removeEventListener("keydown", handleKeyDown, true);
  }, [
    osType,
    state.playHistoryWhenOverlayClosed,
    state.playPauseHotkey,
    state.stopHotkey,
    stopPlayback,
    togglePlayback,
  ]);

  const streamLengthPending =
    state.chunks.length > 0 &&
    state.totalChunks === 0 &&
    state.status !== "completed" &&
    state.status !== "error" &&
    state.status !== "stopped";
  const seekValue =
    !streamLengthPending && playbackDuration > 0
      ? Math.min(1, Math.max(0, playbackProgress))
      : 0;
  const seekPercent = seekValue * 100;
  const seekPlayback = useCallback(
    (event: ChangeEvent<HTMLInputElement>) => {
      if (!playerRef.current || playbackDuration <= 0) {
        return;
      }
      const nextProgress = Number(event.target.value);
      if (!Number.isFinite(nextProgress)) {
        return;
      }
      const actualProgress = playerRef.current.seek(nextProgress);
      if (actualProgress !== null) {
        setPlaybackProgress(actualProgress);
      }
    },
    [playbackDuration],
  );
  const waitingForChunk =
    desiredPlayingRef.current &&
    !isPlaying &&
    state.status !== "idle" &&
    state.status !== "completed" &&
    state.status !== "error" &&
    state.status !== "stopped" &&
    (state.chunks.length === 0 ||
      (activeChunkIndex !== null &&
        !state.chunks.some((chunk) => chunk.index > activeChunkIndex)));
  const label = statusLabel(t, state, isPlaying, waitingForChunk);
  const visibleError = state.error ?? playbackError;
  const canPlay =
    state.status !== "stopped" &&
    state.status !== "error" &&
    (state.status !== "completed" ||
      isPlaying ||
      desiredPlayingRef.current) &&
    (state.chunks.length > 0 ||
      state.status === "loading" ||
      state.status === "retrying");
  const canStop =
    Boolean(state.operationId) &&
    state.status !== "idle" &&
    state.status !== "stopped" &&
    state.status !== "error" &&
    (state.status !== "completed" ||
      isPlaying ||
      desiredPlayingRef.current);
  const playHotkeyLabel = formatPreviewHotkeyForDisplay(
    state.playPauseHotkey,
    osType,
  );
  const stopHotkeyLabel = formatPreviewHotkeyForDisplay(
    state.stopHotkey,
    osType,
  );
  const showVoice =
    Boolean(state.voice) &&
    state.voice.localeCompare(state.model, undefined, {
      sensitivity: "accent",
    }) !== 0;
  const identityTitle = [
    state.provider || "AivoRelay",
    state.model &&
      t("textToSpeech.overlayPlayer.activeModel", { model: state.model }),
    showVoice &&
      t("textToSpeech.overlayPlayer.activeVoice", { voice: state.voice }),
  ]
    .filter(Boolean)
    .join(" · ");
  const nextRate = nextPlaybackRate(playbackRate);
  const playbackRateLabel = formatPlaybackRate(playbackRate);

  return (
    <main className={`tts-overlay tts-overlay--${state.status}`}>
      <div className="tts-overlay__header" data-tauri-drag-region>
        <div className="tts-overlay__identity" data-tauri-drag-region>
          <span className="tts-overlay__pulse" aria-hidden="true" />
          <div data-tauri-drag-region>
            <div className="tts-overlay__status" data-tauri-drag-region>
              {label}
            </div>
            <div
              className="tts-overlay__provider"
              data-tauri-drag-region
              title={identityTitle}
            >
              <span
                className="tts-overlay__provider-name"
                data-tauri-drag-region
              >
                {state.provider || "AivoRelay"}
              </span>
              {state.model && (
                <>
                  <span
                    className="tts-overlay__identity-separator"
                    aria-hidden="true"
                  >
                    ·
                  </span>
                  <span data-tauri-drag-region>
                    {t("textToSpeech.overlayPlayer.activeModel", {
                      model: state.model,
                    })}
                  </span>
                </>
              )}
              {showVoice && (
                <>
                  <span
                    className="tts-overlay__identity-separator"
                    aria-hidden="true"
                  >
                    ·
                  </span>
                  <span data-tauri-drag-region>
                    {t("textToSpeech.overlayPlayer.activeVoice", {
                      voice: state.voice,
                    })}
                  </span>
                </>
              )}
            </div>
          </div>
        </div>
        <div className="tts-overlay__header-actions">
          <button
            type="button"
            className="tts-overlay__close"
            onClick={closeOverlay}
            aria-label={t("common.close")}
            title={t("common.close")}
          >
            <span aria-hidden="true">×</span>
          </button>
        </div>
      </div>

      {state.textPreview && (
        <p className="tts-overlay__preview" title={state.textPreview}>
          {state.textPreview}
        </p>
      )}

      <input
        type="range"
        className="tts-overlay__progress"
        aria-label={t("textToSpeech.overlayPlayer.progress")}
        min={0}
        max={1}
        step={playbackDuration > 0 ? 0.001 : 1}
        value={seekValue}
        disabled={streamLengthPending || playbackDuration <= 0}
        onChange={seekPlayback}
        style={{ "--tts-seek-progress": `${seekPercent}%` } as CSSProperties}
      />

      {visibleError && (
        <div className="tts-overlay__error" role="alert">
          {visibleError}
        </div>
      )}

      <div className="tts-overlay__actions">
        <button
          type="button"
          className="tts-overlay__button tts-overlay__button--primary"
          onClick={togglePlayback}
          disabled={!canPlay}
          title={
            playHotkeyLabel
              ? t("textToSpeech.overlayPlayer.playPauseWithHotkey", {
                  hotkey: playHotkeyLabel,
                })
              : t("textToSpeech.overlayPlayer.playPause")
          }
        >
          <span aria-hidden="true">{isPlaying ? "Ⅱ" : "▶"}</span>
          {isPlaying
            ? t("textToSpeech.overlayPlayer.pause")
            : t("textToSpeech.overlayPlayer.play")}
          {playHotkeyLabel && <kbd>{playHotkeyLabel}</kbd>}
        </button>
        <button
          type="button"
          className="tts-overlay__button"
          onClick={stopPlayback}
          disabled={!canStop}
          title={
            stopHotkeyLabel
              ? t("textToSpeech.overlayPlayer.stopWithHotkey", {
                  hotkey: stopHotkeyLabel,
                })
              : t("textToSpeech.overlayPlayer.stop")
          }
        >
          <span className="tts-overlay__stop-icon" aria-hidden="true" />
          {t("textToSpeech.overlayPlayer.stop")}
          {stopHotkeyLabel && <kbd>{stopHotkeyLabel}</kbd>}
        </button>
        <button
          type="button"
          className="tts-overlay__button tts-overlay__rate"
          onClick={cyclePlaybackRate}
          aria-label={t("textToSpeech.overlayPlayer.playbackRate", {
            rate: playbackRateLabel,
            nextRate: formatPlaybackRate(nextRate),
          })}
          title={t("textToSpeech.overlayPlayer.playbackRateDescription")}
        >
          {playbackRateLabel}
        </button>
      </div>
    </main>
  );
}
