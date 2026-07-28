import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { type as getOsType } from "@tauri-apps/plugin-os";
import {
  type ChangeEvent,
  type CSSProperties,
  useCallback,
  useEffect,
  useMemo,
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
  applyPlaybackRate,
  DEFAULT_PLAYBACK_RATE,
  formatPlaybackRate,
  nextPlaybackRate,
  type PlaybackRate,
} from "../lib/utils/playbackRate";

type TtsStatus =
  | "idle"
  | "loading"
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
};

type UnknownRecord = Record<string, unknown>;

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
};

const VALID_STATUSES = new Set<TtsStatus>([
  "idle",
  "loading",
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
          return path && index !== null ? { index, path } : null;
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
  const [playbackTime, setPlaybackTime] = useState(0);
  const [playbackDuration, setPlaybackDuration] = useState(0);
  const [playbackError, setPlaybackError] = useState<string | null>(null);
  const [playbackRate, setPlaybackRate] = useState<PlaybackRate>(
    DEFAULT_PLAYBACK_RATE,
  );
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const playbackRateRef = useRef<PlaybackRate>(DEFAULT_PLAYBACK_RATE);
  const stateRef = useRef(state);
  const desiredPlayingRef = useRef(false);
  const activeChunkIndexRef = useRef<number | null>(null);
  const operationIdRef = useRef("");
  const lastLoggedProviderErrorRef = useRef("");
  const playChunkRef = useRef<((chunk: TtsChunk) => Promise<void>) | null>(
    null,
  );

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

  const resetAudio = useCallback(() => {
    const audio = audioRef.current;
    if (audio) {
      audio.pause();
      audio.removeAttribute("src");
      audio.load();
    }
    audioRef.current = null;
    activeChunkIndexRef.current = null;
    setActiveChunkIndex(null);
    setIsPlaying(false);
    setPlaybackTime(0);
    setPlaybackDuration(0);
  }, []);

  const findNextChunk = useCallback((afterIndex: number | null) => {
    const chunks = stateRef.current.chunks;
    if (afterIndex === null) {
      return chunks[0] ?? null;
    }
    return chunks.find((chunk) => chunk.index > afterIndex) ?? null;
  }, []);

  const playChunk = useCallback(
    async (chunk: TtsChunk) => {
      if (!desiredPlayingRef.current) {
        return;
      }

      const priorAudio = audioRef.current;
      if (priorAudio) {
        priorAudio.pause();
      }

      const audio = new Audio(convertFileSrc(chunk.path, "asset"));
      audio.preload = "auto";
      applyPlaybackRate(audio, playbackRateRef.current);
      audioRef.current = audio;
      activeChunkIndexRef.current = chunk.index;
      setActiveChunkIndex(chunk.index);
      setPlaybackTime(0);
      setPlaybackDuration(0);
      setPlaybackError(null);

      const updateTimeline = () => {
        if (audioRef.current !== audio) {
          return;
        }
        setPlaybackTime(
          Number.isFinite(audio.currentTime) ? audio.currentTime : 0,
        );
        setPlaybackDuration(
          Number.isFinite(audio.duration) ? audio.duration : 0,
        );
      };
      audio.addEventListener("loadedmetadata", updateTimeline);
      audio.addEventListener("durationchange", updateTimeline);
      audio.addEventListener("timeupdate", updateTimeline);
      audio.addEventListener(
        "ended",
        () => {
          if (audioRef.current !== audio) {
            return;
          }
          setIsPlaying(false);
          updateTimeline();
          const nextChunk = findNextChunk(chunk.index);
          if (nextChunk && desiredPlayingRef.current) {
            void playChunkRef.current?.(nextChunk);
            return;
          }
          audioRef.current = null;
          const completedOrdinal =
            stateRef.current.chunks.findIndex(
              (candidate) => candidate.index === chunk.index,
            ) + 1;
          if (
            stateRef.current.status === "completed" ||
            (stateRef.current.totalChunks > 0 &&
              completedOrdinal >= stateRef.current.totalChunks)
          ) {
            desiredPlayingRef.current = false;
            reportPlaybackState("completed", chunk.index);
          }
        },
        { once: true },
      );
      audio.addEventListener(
        "error",
        () => {
          if (audioRef.current !== audio) {
            return;
          }
          const message = t("textToSpeech.overlayPlayer.chunkPlaybackError");
          desiredPlayingRef.current = false;
          setIsPlaying(false);
          setPlaybackError(message);
          console.error(
            `TTS playback error at chunk ${chunk.index + 1}:`,
            audio.error,
          );
          reportPlaybackState("paused", chunk.index);
        },
        { once: true },
      );

      try {
        await audio.play();
        if (audioRef.current !== audio) {
          audio.pause();
          return;
        }
        setIsPlaying(true);
        reportPlaybackState("playing", chunk.index);
      } catch (error) {
        if (audioRef.current !== audio) {
          return;
        }
        desiredPlayingRef.current = false;
        setIsPlaying(false);
        setPlaybackError(t("textToSpeech.overlayPlayer.playbackStartError"));
        console.error("Unable to start TTS playback:", error);
        reportPlaybackState("paused", chunk.index);
      }
    },
    [findNextChunk, reportPlaybackState, t],
  );
  playChunkRef.current = playChunk;

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
      } else if (
        next.status === "completed" &&
        !audioRef.current &&
        activeChunkIndexRef.current !== null &&
        !next.chunks.some(
          (chunk) => chunk.index > (activeChunkIndexRef.current ?? -1),
        )
      ) {
        desiredPlayingRef.current = false;
        reportPlaybackState("completed", activeChunkIndexRef.current);
      } else if (
        desiredPlayingRef.current &&
        !audioRef.current &&
        next.chunks.length > 0
      ) {
        const nextChunk =
          activeChunkIndexRef.current === null
            ? next.chunks[0]
            : next.chunks.find(
                (chunk) => chunk.index > (activeChunkIndexRef.current ?? -1),
              );
        if (nextChunk) {
          void playChunk(nextChunk);
        }
      }
    },
    [playChunk, reportPlaybackState, resetAudio],
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
    const audio = audioRef.current;
    if (audio && !audio.paused) {
      desiredPlayingRef.current = false;
      audio.pause();
      setIsPlaying(false);
      reportPlaybackState("paused");
      return;
    }

    desiredPlayingRef.current = true;
    setPlaybackError(null);
    if (audio && activeChunkIndexRef.current !== null && !audio.ended) {
      void audio
        .play()
        .then(() => {
          if (audioRef.current !== audio || !desiredPlayingRef.current) {
            audio.pause();
            return;
          }
          setIsPlaying(true);
          reportPlaybackState("playing");
        })
        .catch((error) => {
          if (audioRef.current !== audio || !desiredPlayingRef.current) {
            return;
          }
          desiredPlayingRef.current = false;
          setPlaybackError(t("textToSpeech.overlayPlayer.playbackStartError"));
          console.error("Unable to resume TTS playback:", error);
        });
      return;
    }

    const nextChunk = findNextChunk(activeChunkIndexRef.current);
    if (nextChunk) {
      void playChunk(nextChunk);
    }
  }, [findNextChunk, playChunk, reportPlaybackState, t]);

  const stopPlayback = useCallback(() => {
    const operationId = operationIdRef.current;
    desiredPlayingRef.current = false;
    resetAudio();
    reportPlaybackState("stopped");
    if (!operationId) {
      return;
    }
    void invoke("cancel_tts_operation", { operationId }).catch((error) => {
      const message = t("textToSpeech.overlayPlayer.cancelError");
      setPlaybackError(message);
      console.error(message, error);
    });
  }, [reportPlaybackState, resetAudio, t]);

  const cyclePlaybackRate = useCallback(() => {
    const nextRate = nextPlaybackRate(playbackRateRef.current);
    playbackRateRef.current = nextRate;
    setPlaybackRate(nextRate);
    const audio = audioRef.current;
    if (audio) {
      applyPlaybackRate(audio, nextRate);
    }
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

  const currentPosition = useMemo(() => {
    if (activeChunkIndex !== null) {
      const ordinal = state.chunks.findIndex(
        (chunk) => chunk.index === activeChunkIndex,
      );
      return ordinal >= 0 ? ordinal + 1 : activeChunkIndex + 1;
    }
    return state.currentChunk;
  }, [activeChunkIndex, state.chunks, state.currentChunk]);
  const totalChunks = Math.max(state.totalChunks, state.chunks.length);
  const progress =
    totalChunks > 0
      ? Math.min(100, Math.max(0, (currentPosition / totalChunks) * 100))
      : 0;
  const seekValue =
    playbackDuration > 0
      ? Math.min(playbackDuration, Math.max(0, playbackTime))
      : progress;
  const seekMaximum = playbackDuration > 0 ? playbackDuration : 100;
  const seekPercent =
    seekMaximum > 0
      ? Math.min(100, Math.max(0, (seekValue / seekMaximum) * 100))
      : 0;
  const seekPlayback = useCallback(
    (event: ChangeEvent<HTMLInputElement>) => {
      const audio = audioRef.current;
      if (!audio || playbackDuration <= 0) {
        return;
      }
      const nextTime = Number(event.target.value);
      if (!Number.isFinite(nextTime)) {
        return;
      }
      const boundedTime = Math.min(playbackDuration, Math.max(0, nextTime));
      audio.currentTime = boundedTime;
      setPlaybackTime(boundedTime);
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
    (isPlaying ||
      Boolean(audioRef.current && !audioRef.current.ended) ||
      activeChunkIndex === null ||
      state.chunks.some((chunk) => chunk.index > activeChunkIndex)) &&
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
      Boolean(audioRef.current && !audioRef.current.ended));
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
        {totalChunks > 0 && (
          <div
            className="tts-overlay__count"
            aria-label={t("textToSpeech.overlayPlayer.chunkProgress", {
              current: Math.min(currentPosition, totalChunks),
              total: totalChunks,
            })}
          >
            {Math.min(currentPosition, totalChunks)} / {totalChunks}
          </div>
        )}
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
        max={seekMaximum}
        step={playbackDuration > 0 ? 0.01 : 1}
        value={seekValue}
        disabled={playbackDuration <= 0}
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
