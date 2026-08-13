import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { type as getOsType } from "@tauri-apps/plugin-os";
import {
  type ChangeEvent,
  type CSSProperties,
  type DragEvent as ReactDragEvent,
  type PointerEvent as ReactPointerEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import {
  ChevronDown,
  ChevronUp,
  GripVertical,
  ListMusic,
  SkipForward,
  Trash2,
} from "lucide-react";
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
  normalizePlaybackRate,
  PLAYBACK_RATES,
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

type TtsQueueItemStatus = "active" | "pending" | "failed";

type TtsQueueItem = {
  id: string;
  textPreview: string;
  sourceLabel: string;
  status: TtsQueueItemStatus;
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
  queueEnabled: boolean;
  queueGeneration: string;
  queueItems: TtsQueueItem[];
  activeQueueItemId: string | null;
};

type UnknownRecord = Record<string, unknown>;

type TtsOverlayNotice = {
  revision: number;
  message: string;
};

const overlayWindow = getCurrentWindow();
const OVERLAY_NOTICE_DURATION_MS = 8_000;

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
  autoHideDelaySeconds: 2,
  playbackPitch: 1,
  playbackEffect: "none",
  queueEnabled: false,
  queueGeneration: "1",
  queueItems: [],
  activeQueueItemId: null,
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
  const rawQueueItems = data.queueItems ?? data.queue_items;
  const queueItems = Array.isArray(rawQueueItems)
    ? rawQueueItems
        .map((rawItem): TtsQueueItem | null => {
          const item = asRecord(rawItem);
          if (!item) return null;
          const id = readString(item, "id", "id");
          const status = readString(item, "status", "status");
          if (!id || !["active", "pending", "failed"].includes(status)) {
            return null;
          }
          return {
            id,
            textPreview: readString(item, "textPreview", "text_preview"),
            sourceLabel: readString(item, "sourceLabel", "source_label"),
            status: status as TtsQueueItemStatus,
          };
        })
        .filter((item): item is TtsQueueItem => item !== null)
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
          2,
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
    queueEnabled: readBoolean(data, "queueEnabled", "queue_enabled", false),
    queueGeneration:
      readString(data, "queueGeneration", "queue_generation") || "1",
    queueItems,
    activeQueueItemId:
      readString(data, "activeQueueItemId", "active_queue_item_id") || null,
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

function formatPlaybackTime(seconds: number): string {
  const safeSeconds = Math.max(0, Math.floor(seconds));
  const minutes = Math.floor(safeSeconds / 60);
  return `${minutes}:${String(safeSeconds % 60).padStart(2, "0")}`;
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
  const [playbackProgress, setPlaybackProgress] = useState(0);
  const [completedPlaybackOperationId, setCompletedPlaybackOperationId] =
    useState("");
  const [autoHideActivityRevision, setAutoHideActivityRevision] = useState(0);
  const [pointerInteractionActive, setPointerInteractionActive] =
    useState(false);
  const [playbackError, setPlaybackError] = useState<string | null>(null);
  const [playbackRate, setPlaybackRate] = useState<number>(
    DEFAULT_PLAYBACK_RATE,
  );
  const [rateSliderOpen, setRateSliderOpen] = useState(false);
  const [queueExpanded, setQueueExpanded] = useState(false);
  const [draggedQueueItemId, setDraggedQueueItemId] = useState<string | null>(
    null,
  );
  const [queueDropTargetId, setQueueDropTargetId] = useState<string | null>(
    null,
  );
  const [queueMutation, setQueueMutation] = useState<string | null>(null);
  const [queueError, setQueueError] = useState<string | null>(null);
  const [overlayNotice, setOverlayNotice] =
    useState<TtsOverlayNotice | null>(null);
  const playerRef = useRef<GaplessTtsPlayer | null>(null);
  const rateSliderRef = useRef<HTMLInputElement | null>(null);
  const playbackRateRef = useRef<number>(DEFAULT_PLAYBACK_RATE);
  const playbackProgressRef = useRef(0);
  const stateRef = useRef(state);
  const desiredPlayingRef = useRef(false);
  const activeChunkIndexRef = useRef<number | null>(null);
  const operationIdRef = useRef("");
  const queueGenerationRef = useRef("1");
  const lastLoggedProviderErrorRef = useRef("");
  const queueMutationRef = useRef(false);

  useEffect(() => {
    stateRef.current = state;
  }, [state]);

  const reportPlaybackState = useCallback(
    (
      status: "playing" | "paused" | "stopped" | "completed" | "error",
      chunk?: number,
      reportedOperationId?: string,
      reportedQueueGeneration?: string,
    ) => {
      const operationId = reportedOperationId ?? operationIdRef.current;
      if (!operationId) {
        return;
      }
      void invoke("tts_overlay_playback_state", {
        operationId,
        queueGeneration:
          reportedQueueGeneration ?? queueGenerationRef.current,
        status,
        currentChunk: chunk ?? activeChunkIndexRef.current,
      }).catch((error) => {
        console.error("Unable to report TTS overlay playback state:", error);
      });
    },
    [],
  );

  const getPlayer = useCallback(() => {
    const callbackOperationId = operationIdRef.current;
    const callbackQueueGeneration = queueGenerationRef.current;
    const callbacks = {
      onSnapshot: (snapshot: GaplessPlaybackSnapshot) => {
        activeChunkIndexRef.current = snapshot.chunkIndex;
        setActiveChunkIndex(snapshot.chunkIndex);
        setIsPlaying(snapshot.isPlaying);
        setPlaybackTime(snapshot.playbackTime);
        setPlaybackDuration(snapshot.playbackDuration);
        playbackProgressRef.current = snapshot.playbackProgress;
        setPlaybackProgress(snapshot.playbackProgress);
      },
      onChunkStart: (chunkIndex: number) => {
        activeChunkIndexRef.current = chunkIndex;
        setActiveChunkIndex(chunkIndex);
        setPlaybackError(null);
        reportPlaybackState(
          "playing",
          chunkIndex,
          callbackOperationId,
          callbackQueueGeneration,
        );
      },
      onCompleted: (chunkIndex: number) => {
        desiredPlayingRef.current = false;
        setIsPlaying(false);
        setCompletedPlaybackOperationId(callbackOperationId);
        reportPlaybackState(
          "completed",
          chunkIndex,
          callbackOperationId,
          callbackQueueGeneration,
        );
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
        reportPlaybackState(
          "error",
          chunkIndex,
          callbackOperationId,
          callbackQueueGeneration,
        );
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
    setPlaybackTime(0);
    setPlaybackDuration(0);
    playbackProgressRef.current = 0;
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

      const previousState = stateRef.current;
      const previousOperationId = operationIdRef.current;

      const operationChanged = next.operationId !== previousOperationId;
      const activeQueueItemChanged =
        next.activeQueueItemId !== previousState.activeQueueItemId;
      // Active -> null after normal completion only removes queue ownership;
      // the completed operation and its retained audio are still the same.
      const queueItemActivatedOrReplaced =
        activeQueueItemChanged && next.activeQueueItemId !== null;

      const assignedOperationToSameQueueItem =
        operationChanged &&
        previousOperationId === "" &&
        next.operationId !== "" &&
        next.activeQueueItemId !== null &&
        next.activeQueueItemId === previousState.activeQueueItemId;

      const logicalPlaybackItemChanged =
        queueItemActivatedOrReplaced ||
        (operationChanged && !assignedOperationToSameQueueItem);

      if (operationChanged || queueItemActivatedOrReplaced) {
        resetAudio();
        operationIdRef.current = next.operationId;

        if (logicalPlaybackItemChanged) {
          desiredPlayingRef.current =
            next.autoplay &&
            (next.status === "loading" ||
              next.status === "preprocessing" ||
              next.status === "retrying" ||
              next.status === "ready");
        }

        setPlaybackError(null);
        setCompletedPlaybackOperationId("");
        setRateSliderOpen(false);
        lastLoggedProviderErrorRef.current = "";
      }

      queueGenerationRef.current = next.queueGeneration;
      stateRef.current = next;
      setState(next);
      if (!next.queueEnabled) {
        setQueueExpanded(false);
        setDraggedQueueItemId(null);
        setQueueDropTargetId(null);
        setQueueError(null);
      }

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

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void listen<string>("tts-overlay-notice", (event) => {
      if (!active) return;
      const message = safeErrorMessage(event.payload);
      if (!message) return;
      setOverlayNotice((current) => ({
        revision: (current?.revision ?? 0) + 1,
        message,
      }));
    })
      .then((dispose) => {
        if (active) unlisten = dispose;
        else dispose();
      })
      .catch((error) => {
        console.error("Unable to listen for TTS overlay notices:", error);
      });
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!overlayNotice) return;
    const revision = overlayNotice.revision;
    const timeout = window.setTimeout(() => {
      setOverlayNotice((current) =>
        current?.revision === revision ? null : current,
      );
    }, OVERLAY_NOTICE_DURATION_MS);
    return () => window.clearTimeout(timeout);
  }, [overlayNotice]);

  const togglePlayback = useCallback(() => {
    const player = getPlayer();
    if (isPlaying || desiredPlayingRef.current) {
      desiredPlayingRef.current = false;
      player.pause();
      reportPlaybackState("paused");
      return;
    }

    desiredPlayingRef.current = true;
    setCompletedPlaybackOperationId("");
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
    if (
      stateRef.current.status === "completed" &&
      playbackProgressRef.current >= 1
    ) {
      const restartedProgress = player.seek(0);
      if (restartedProgress !== null) {
        playbackProgressRef.current = restartedProgress;
        setPlaybackProgress(restartedProgress);
      }
    }
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
      queueGeneration: queueGenerationRef.current,
    }).catch((error) => {
      const message = t("textToSpeech.overlayPlayer.cancelError");
      setPlaybackError(message);
      console.error(message, error);
    });
  }, [reportPlaybackState, resetAudio, t]);

  const closeOverlay = useCallback(() => {
    setQueueExpanded(false);
    setOverlayNotice(null);
    stopPlayback();
    void overlayWindow.hide().catch((error) => {
      console.error("Unable to hide the TTS overlay:", error);
    });
  }, [stopPlayback]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void overlayWindow
      .onCloseRequested((event) => {
        event.preventDefault();
        if (!disposed) {
          closeOverlay();
        }
      })
      .then((dispose) => {
        if (disposed) dispose();
        else unlisten = dispose;
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [closeOverlay]);

  const recordOverlayActivity = useCallback(() => {
    if (stateRef.current.status === "completed") {
      setAutoHideActivityRevision((revision) => revision + 1);
    }
  }, []);

  const beginPointerInteraction = useCallback(() => {
    setPointerInteractionActive(true);
    recordOverlayActivity();
  }, [recordOverlayActivity]);

  const endPointerInteraction = useCallback(() => {
    setPointerInteractionActive(false);
    recordOverlayActivity();
  }, [recordOverlayActivity]);

  const handleDragGripPointerDown = useCallback(
    (event: ReactPointerEvent<HTMLButtonElement>) => {
      if (event.button !== 0) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      beginPointerInteraction();
      void overlayWindow
        .startDragging()
        .catch((error) => {
          console.error("Unable to drag the TTS overlay:", error);
        })
        .finally(endPointerInteraction);
    },
    [beginPointerInteraction, endPointerInteraction],
  );

  useEffect(() => {
    void invoke("set_tts_listen_queue_expanded", {
      expanded: queueExpanded,
    }).catch((error) => {
      console.error("Unable to resize the TTS Listen Later queue:", error);
    });
  }, [queueExpanded]);

  useEffect(
    () => () => {
      void invoke("set_tts_listen_queue_expanded", {
        expanded: false,
      }).catch(() => undefined);
    },
    [],
  );

  const runQueueMutation = useCallback(
    async (mutation: string, command: string, args: UnknownRecord) => {
      if (queueMutationRef.current) return false;
      queueMutationRef.current = true;
      setQueueMutation(mutation);
      setQueueError(null);
      recordOverlayActivity();
      try {
        await invoke(command, args);
        return true;
      } catch (error) {
        setQueueError(t("textToSpeech.overlayPlayer.queueUpdateError"));
        console.error("Unable to update the TTS Listen Later queue:", error);
        return false;
      } finally {
        queueMutationRef.current = false;
        setQueueMutation(null);
      }
    },
    [recordOverlayActivity, t],
  );

  const reorderPendingQueue = useCallback(
    (itemIds: string[]) =>
      runQueueMutation("reorder", "reorder_tts_listen_queue", {
        itemIds,
      }),
    [runQueueMutation],
  );

  const movePendingQueueItem = useCallback(
    (itemId: string, offset: -1 | 1) => {
      const pendingIds = stateRef.current.queueItems
        .filter((item) => item.id !== stateRef.current.activeQueueItemId)
        .map((item) => item.id);
      const currentIndex = pendingIds.indexOf(itemId);
      const nextIndex = currentIndex + offset;
      if (currentIndex < 0 || nextIndex < 0 || nextIndex >= pendingIds.length) {
        return;
      }
      const [moved] = pendingIds.splice(currentIndex, 1);
      pendingIds.splice(nextIndex, 0, moved);
      void reorderPendingQueue(pendingIds);
    },
    [reorderPendingQueue],
  );

  const handleQueueDragStart = useCallback(
    (event: ReactDragEvent<HTMLDivElement>, itemId: string) => {
      if (itemId === stateRef.current.activeQueueItemId || queueMutation) {
        event.preventDefault();
        return;
      }
      setDraggedQueueItemId(itemId);
      setQueueDropTargetId(null);
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData("text/plain", itemId);
    },
    [queueMutation],
  );

  const handleQueueDrop = useCallback(
    (event: ReactDragEvent<HTMLDivElement>, targetItemId: string) => {
      event.preventDefault();
      event.stopPropagation();
      const draggedId =
        draggedQueueItemId || event.dataTransfer.getData("text/plain");
      setDraggedQueueItemId(null);
      setQueueDropTargetId(null);
      if (
        !draggedId ||
        draggedId === targetItemId ||
        draggedId === stateRef.current.activeQueueItemId ||
        targetItemId === stateRef.current.activeQueueItemId
      ) {
        return;
      }
      const pendingIds = stateRef.current.queueItems
        .filter((item) => item.id !== stateRef.current.activeQueueItemId)
        .map((item) => item.id);
      const sourceIndex = pendingIds.indexOf(draggedId);
      const targetIndex = pendingIds.indexOf(targetItemId);
      if (sourceIndex < 0 || targetIndex < 0) return;
      const [moved] = pendingIds.splice(sourceIndex, 1);
      pendingIds.splice(targetIndex, 0, moved);
      void reorderPendingQueue(pendingIds);
    },
    [draggedQueueItemId, reorderPendingQueue],
  );

  const handleQueueDropAtEnd = useCallback(
    (event: ReactDragEvent<HTMLDivElement>) => {
      event.preventDefault();
      const draggedId =
        draggedQueueItemId || event.dataTransfer.getData("text/plain");
      setDraggedQueueItemId(null);
      setQueueDropTargetId(null);
      if (!draggedId || draggedId === stateRef.current.activeQueueItemId) {
        return;
      }
      const pendingIds = stateRef.current.queueItems
        .filter((item) => item.id !== stateRef.current.activeQueueItemId)
        .map((item) => item.id);
      const sourceIndex = pendingIds.indexOf(draggedId);
      if (sourceIndex < 0 || sourceIndex === pendingIds.length - 1) return;
      const [moved] = pendingIds.splice(sourceIndex, 1);
      pendingIds.push(moved);
      void reorderPendingQueue(pendingIds);
    },
    [draggedQueueItemId, reorderPendingQueue],
  );

  const removeQueueItem = useCallback(
    (itemId: string) => {
      void runQueueMutation("remove", "remove_tts_listen_queue_item", {
        itemId,
      });
    },
    [runQueueMutation],
  );

  const skipQueueItem = useCallback(() => {
    const itemId = stateRef.current.activeQueueItemId;
    if (!itemId) return;
    desiredPlayingRef.current = false;
    resetAudio();
    setCompletedPlaybackOperationId("");
    void runQueueMutation("skip", "skip_tts_listen_queue_item", {
      itemId,
    });
  }, [resetAudio, runQueueMutation]);

  const clearQueue = useCallback(() => {
    if (stateRef.current.activeQueueItemId) {
      desiredPlayingRef.current = false;
      resetAudio();
      setCompletedPlaybackOperationId("");
    }
    void runQueueMutation("clear", "clear_tts_listen_queue", {
      queueGeneration: queueGenerationRef.current,
    });
  }, [resetAudio, runQueueMutation]);

  useEffect(() => {
    if (
      !state.autoHideEnabled ||
      state.status !== "completed" ||
      !state.operationId ||
      pointerInteractionActive ||
      overlayNotice !== null ||
      completedPlaybackOperationId !== state.operationId
    ) {
      return;
    }

    const completedOperationId = state.operationId;
    const timeout = window.setTimeout(() => {
      setQueueExpanded(false);
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
    autoHideActivityRevision,
    completedPlaybackOperationId,
    overlayNotice,
    pointerInteractionActive,
    resetAudio,
    state.autoHideDelaySeconds,
    state.autoHideEnabled,
    state.operationId,
    state.status,
  ]);

  const applyOverlayPlaybackRate = useCallback(
    (requestedRate: number) => {
      const nextRate = normalizePlaybackRate(requestedRate);
      playbackRateRef.current = nextRate;
      setPlaybackRate(nextRate);
      playerRef.current?.setPlaybackRate(nextRate);
      recordOverlayActivity();
    },
    [recordOverlayActivity],
  );

  const changePlaybackRate = useCallback(
    (event: ChangeEvent<HTMLInputElement>) => {
      const requestedIndex = Number(event.target.value);
      const nextIndex = Math.min(
        PLAYBACK_RATES.length - 1,
        Math.max(0, Math.round(requestedIndex)),
      );
      applyOverlayPlaybackRate(PLAYBACK_RATES[nextIndex]);
    },
    [applyOverlayPlaybackRate],
  );

  const changePlaybackRateWithWheel = useCallback(
    (event: WheelEvent) => {
      const wheelDelta =
        Math.abs(event.deltaY) >= Math.abs(event.deltaX)
          ? event.deltaY
          : event.deltaX;
      if (wheelDelta === 0) return;
      event.preventDefault();
      event.stopPropagation();
      const currentRate = normalizePlaybackRate(playbackRateRef.current);
      const currentIndex = PLAYBACK_RATES.indexOf(currentRate);
      const nextIndex = Math.min(
        PLAYBACK_RATES.length - 1,
        Math.max(0, currentIndex + (wheelDelta < 0 ? 1 : -1)),
      );
      applyOverlayPlaybackRate(PLAYBACK_RATES[nextIndex]);
    },
    [applyOverlayPlaybackRate],
  );

  useEffect(() => {
    if (!rateSliderOpen) return;
    const slider = rateSliderRef.current;
    if (!slider) return;
    slider.addEventListener("wheel", changePlaybackRateWithWheel, {
      passive: false,
    });
    const frame = window.requestAnimationFrame(() => {
      slider.focus({ preventScroll: true });
    });
    return () => {
      window.cancelAnimationFrame(frame);
      slider.removeEventListener("wheel", changePlaybackRateWithWheel);
    };
  }, [changePlaybackRateWithWheel, rateSliderOpen]);

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
      recordOverlayActivity();
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
    recordOverlayActivity,
    state.playHistoryWhenOverlayClosed,
    state.playPauseHotkey,
    state.stopHotkey,
    stopPlayback,
    togglePlayback,
  ]);

  const streamLengthPending =
    state.totalChunks === 0 &&
    state.status !== "idle" &&
    state.status !== "completed" &&
    state.status !== "error" &&
    state.status !== "stopped";
  const seekValue =
    !streamLengthPending && playbackDuration > 0
      ? Math.min(1, Math.max(0, playbackProgress))
      : 0;
  const seekPercent = seekValue * 100;
  const readyChunkCount = state.chunks.length;
  const plannedChunkCount =
    state.totalChunks > 0
      ? state.totalChunks
      : state.status === "completed"
        ? readyChunkCount
        : 0;
  const readyProgress =
    plannedChunkCount > 0
      ? Math.min(1, readyChunkCount / plannedChunkCount)
      : state.status === "completed" && readyChunkCount > 0
        ? 1
        : 0;
  const readyPercent = Math.max(seekPercent, readyProgress * 100);
  const pendingChunkCount = Math.max(
    0,
    plannedChunkCount - readyChunkCount,
  );
  const allChunksReady =
    state.status === "completed" ||
    (plannedChunkCount > 0 && readyChunkCount >= plannedChunkCount);
  const elapsedLabel = formatPlaybackTime(playbackTime);
  const durationLabel = formatPlaybackTime(playbackDuration);
  const timelineTimeLabel = allChunksReady
    ? `${elapsedLabel} / ${durationLabel}`
    : t("textToSpeech.overlayPlayer.generatedTime", {
        elapsed: elapsedLabel,
        duration: durationLabel,
      });
  const chunkAvailabilityLabel =
    plannedChunkCount > 0
      ? t("textToSpeech.overlayPlayer.chunksAvailable", {
          ready: readyChunkCount,
          total: plannedChunkCount,
          pending: pendingChunkCount,
        })
      : t("textToSpeech.overlayPlayer.chunksAvailableUnknown", {
          ready: readyChunkCount,
        });
  const seekPlayback = useCallback(
    (event: ChangeEvent<HTMLInputElement>) => {
      if (!playerRef.current || playbackDuration <= 0) {
        return;
      }
      const requestedProgress = Number(event.target.value);
      if (!Number.isFinite(requestedProgress)) {
        return;
      }
      const nextProgress = Math.min(
        requestedProgress,
        allChunksReady ? 1 : readyProgress,
      );
      const actualProgress = playerRef.current.seek(nextProgress);
      if (actualProgress !== null) {
        playbackProgressRef.current = actualProgress;
        setPlaybackProgress(actualProgress);
        recordOverlayActivity();
      }
    },
    [allChunksReady, playbackDuration, readyProgress, recordOverlayActivity],
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
  const playbackRateLabel = formatPlaybackRate(playbackRate);
  const playbackRateIndex = PLAYBACK_RATES.indexOf(
    normalizePlaybackRate(playbackRate),
  );
  const activeQueueIndex = state.queueItems.findIndex(
    (item) => item.id === state.activeQueueItemId,
  );
  const pendingQueueItems = state.queueItems.filter(
    (item) => item.id !== state.activeQueueItemId,
  );

  return (
    <main
      className={`tts-overlay tts-overlay--${state.status}${
        queueExpanded ? " tts-overlay--queue-expanded" : ""
      }`}
      onPointerDown={beginPointerInteraction}
      onPointerUp={endPointerInteraction}
      onPointerCancel={endPointerInteraction}
    >
      <div className="tts-overlay__grip-row">
        <button
          type="button"
          className="tts-overlay__grip"
          aria-label={t("textToSpeech.overlayPlayer.dragToMove")}
          title={t("textToSpeech.overlayPlayer.dragToMove")}
          onPointerDown={handleDragGripPointerDown}
        >
          {Array.from({ length: 6 }).map((_, index) => (
            <span key={index} className="tts-overlay__grip-dot" />
          ))}
        </button>
      </div>

      {overlayNotice && (
        <div className="tts-overlay__notice" role="alert">
          <div>
            <strong>
              {t("textToSpeech.overlayPlayer.inputNoticeTitle")}
            </strong>
            <span>{overlayNotice.message}</span>
          </div>
          <button
            type="button"
            onClick={() => setOverlayNotice(null)}
            aria-label={t("common.close")}
            title={t("common.close")}
          >
            <span aria-hidden="true">×</span>
          </button>
        </div>
      )}

      {queueExpanded && (
        <section
          className="tts-overlay__queue"
          aria-label={t("textToSpeech.overlayPlayer.queue")}
        >
          <div className="tts-overlay__queue-header">
            <div className="tts-overlay__queue-title">
              <ListMusic size={15} strokeWidth={1.8} aria-hidden="true" />
              <span>{t("textToSpeech.overlayPlayer.queue")}</span>
              <span className="tts-overlay__queue-position">
                {t("textToSpeech.overlayPlayer.queuePosition", {
                  current: activeQueueIndex >= 0 ? activeQueueIndex + 1 : 0,
                  total: state.queueItems.length,
                })}
              </span>
            </div>
            <button
              type="button"
              className="tts-overlay__queue-clear"
              onClick={clearQueue}
              disabled={state.queueItems.length === 0 || queueMutation !== null}
              title={t("textToSpeech.overlayPlayer.queueClear")}
            >
              {t("textToSpeech.overlayPlayer.queueClear")}
            </button>
          </div>

          {queueError && (
            <div className="tts-overlay__queue-error" role="alert">
              {queueError}
            </div>
          )}

          {state.queueItems.length === 0 ? (
            <div className="tts-overlay__queue-empty">
              <ListMusic size={22} strokeWidth={1.5} aria-hidden="true" />
              <strong>{t("textToSpeech.overlayPlayer.queueEmptyTitle")}</strong>
              <span>
                {t("textToSpeech.overlayPlayer.queueEmptyDescription")}
              </span>
            </div>
          ) : (
            <div
              className={`tts-overlay__queue-list${
                queueDropTargetId === "__end"
                  ? " tts-overlay__queue-list--drop-at-end"
                  : ""
              }`}
              role="list"
              onDragOver={(event) => {
                event.preventDefault();
                if (event.target === event.currentTarget) {
                  setQueueDropTargetId("__end");
                }
              }}
              onDrop={handleQueueDropAtEnd}
            >
              {state.queueItems.map((item) => {
                const isActive = item.id === state.activeQueueItemId;
                const pendingIndex = pendingQueueItems.findIndex(
                  (pending) => pending.id === item.id,
                );
                const statusLabel = isActive
                  ? item.status === "failed"
                    ? t("textToSpeech.overlayPlayer.queueFailed")
                    : t("textToSpeech.overlayPlayer.queueActive")
                  : t("textToSpeech.overlayPlayer.queuePending");
                return (
                  <div
                    key={item.id}
                    className={`tts-overlay__queue-item tts-overlay__queue-item--${item.status}${
                      draggedQueueItemId === item.id
                        ? " tts-overlay__queue-item--dragging"
                        : ""
                    }${
                      queueDropTargetId === item.id
                        ? " tts-overlay__queue-item--drop-target"
                        : ""
                    }`}
                    role="listitem"
                    draggable={!isActive && queueMutation === null}
                    onDragStart={(event) =>
                      handleQueueDragStart(event, item.id)
                    }
                    onDragEnd={() => {
                      setDraggedQueueItemId(null);
                      setQueueDropTargetId(null);
                    }}
                    onDragOver={(event) => {
                      if (!isActive) {
                        event.preventDefault();
                        event.stopPropagation();
                        if (draggedQueueItemId !== item.id) {
                          setQueueDropTargetId(item.id);
                        }
                      }
                    }}
                    onDrop={(event) => handleQueueDrop(event, item.id)}
                  >
                    <div
                      className="tts-overlay__queue-drag"
                      title={
                        isActive
                          ? statusLabel
                          : t("textToSpeech.overlayPlayer.queueDrag")
                      }
                      aria-hidden="true"
                    >
                      <GripVertical size={15} strokeWidth={1.8} />
                    </div>
                    <div className="tts-overlay__queue-copy">
                      <div className="tts-overlay__queue-meta">
                        <span>{statusLabel}</span>
                        <span
                          title={
                            item.sourceLabel ||
                            t("textToSpeech.overlayPlayer.queueDesktopSource")
                          }
                        >
                          {item.sourceLabel ||
                            t("textToSpeech.overlayPlayer.queueDesktopSource")}
                        </span>
                      </div>
                      <p title={item.textPreview}>{item.textPreview}</p>
                    </div>
                    <div className="tts-overlay__queue-actions">
                      {isActive ? (
                        <button
                          type="button"
                          onClick={skipQueueItem}
                          disabled={queueMutation !== null}
                          aria-label={t("textToSpeech.overlayPlayer.queueSkip")}
                          title={t("textToSpeech.overlayPlayer.queueSkip")}
                        >
                          <SkipForward
                            size={14}
                            strokeWidth={1.8}
                            aria-hidden="true"
                          />
                        </button>
                      ) : (
                        <>
                          <button
                            type="button"
                            onClick={() => movePendingQueueItem(item.id, -1)}
                            disabled={
                              pendingIndex <= 0 || queueMutation !== null
                            }
                            aria-label={t(
                              "textToSpeech.overlayPlayer.queueMoveUp",
                            )}
                            title={t("textToSpeech.overlayPlayer.queueMoveUp")}
                          >
                            <ChevronUp
                              size={14}
                              strokeWidth={1.8}
                              aria-hidden="true"
                            />
                          </button>
                          <button
                            type="button"
                            onClick={() => movePendingQueueItem(item.id, 1)}
                            disabled={
                              pendingIndex < 0 ||
                              pendingIndex >= pendingQueueItems.length - 1 ||
                              queueMutation !== null
                            }
                            aria-label={t(
                              "textToSpeech.overlayPlayer.queueMoveDown",
                            )}
                            title={t(
                              "textToSpeech.overlayPlayer.queueMoveDown",
                            )}
                          >
                            <ChevronDown
                              size={14}
                              strokeWidth={1.8}
                              aria-hidden="true"
                            />
                          </button>
                          <button
                            type="button"
                            onClick={() => removeQueueItem(item.id)}
                            disabled={queueMutation !== null}
                            aria-label={t(
                              "textToSpeech.overlayPlayer.queueRemove",
                            )}
                            title={t("textToSpeech.overlayPlayer.queueRemove")}
                          >
                            <Trash2
                              size={13}
                              strokeWidth={1.8}
                              aria-hidden="true"
                            />
                          </button>
                        </>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </section>
      )}

      <div className="tts-overlay__player">
        <div className="tts-overlay__header">
          <div className="tts-overlay__identity">
            <span className="tts-overlay__pulse" aria-hidden="true" />
            <div>
              <div className="tts-overlay__status">{label}</div>
              <div className="tts-overlay__provider" title={identityTitle}>
                <span className="tts-overlay__provider-name">
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
                    <span>
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
                    <span>
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
            {state.queueEnabled && (
              <button
                type="button"
                className={`tts-overlay__queue-toggle${
                  queueExpanded ? " tts-overlay__queue-toggle--active" : ""
                }`}
                onClick={() => {
                  setQueueError(null);
                  setQueueExpanded((expanded) => !expanded);
                }}
                aria-expanded={queueExpanded}
                aria-label={t(
                  queueExpanded
                    ? "textToSpeech.overlayPlayer.queueCollapse"
                    : "textToSpeech.overlayPlayer.queueExpand",
                )}
                title={t(
                  queueExpanded
                    ? "textToSpeech.overlayPlayer.queueCollapse"
                    : "textToSpeech.overlayPlayer.queueExpand",
                )}
              >
                <ListMusic size={14} strokeWidth={1.8} aria-hidden="true" />
                <span>{state.queueItems.length}</span>
                {queueExpanded ? (
                  <ChevronDown size={12} strokeWidth={1.8} aria-hidden="true" />
                ) : (
                  <ChevronUp size={12} strokeWidth={1.8} aria-hidden="true" />
                )}
              </button>
            )}
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

        <div className="tts-overlay__timeline">
          <input
            type="range"
            className={`tts-overlay__progress${
              streamLengthPending ? " tts-overlay__progress--pending" : ""
            }`}
            aria-label={t("textToSpeech.overlayPlayer.progress")}
            aria-valuetext={`${timelineTimeLabel}. ${chunkAvailabilityLabel}`}
            min={0}
            max={1}
            step={playbackDuration > 0 ? 0.001 : 1}
            value={seekValue}
            disabled={streamLengthPending || playbackDuration <= 0}
            onChange={seekPlayback}
            style={
              {
                "--tts-seek-progress": `${seekPercent}%`,
                "--tts-ready-progress": `${readyPercent}%`,
              } as CSSProperties
            }
          />
          <div className="tts-overlay__timeline-meta" aria-hidden="true">
            <span>{timelineTimeLabel}</span>
            <span>{chunkAvailabilityLabel}</span>
          </div>
        </div>

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
          <div className="tts-overlay__rate-control">
            {rateSliderOpen && (
              <div className="tts-overlay__rate-panel">
                <input
                  ref={rateSliderRef}
                  type="range"
                  className="tts-overlay__rate-slider"
                  aria-label={t(
                    "textToSpeech.overlayPlayer.playbackRateSlider",
                  )}
                  aria-valuetext={playbackRateLabel}
                  min={0}
                  max={PLAYBACK_RATES.length - 1}
                  step={1}
                  value={playbackRateIndex}
                  onChange={changePlaybackRate}
                />
              </div>
            )}
            <button
              type="button"
              className="tts-overlay__button tts-overlay__rate"
              onClick={() => setRateSliderOpen((open) => !open)}
              aria-expanded={rateSliderOpen}
              aria-label={t("textToSpeech.overlayPlayer.playbackRate", {
                rate: playbackRateLabel,
              })}
              title={t("textToSpeech.overlayPlayer.playbackRateDescription")}
            >
              {playbackRateLabel}
            </button>
          </div>
        </div>
      </div>
    </main>
  );
}
