import { convertFileSrc } from "@tauri-apps/api/core";
import {
  prepareTtsPlaybackBuffer,
  type PreparedTtsPlaybackBuffer,
  type TtsPlaybackEffect,
} from "../lib/utils/ttsPlaybackEffects";

export type GaplessTtsChunk = {
  index: number;
  path: string;
  pauseAfterMs: number;
};

export type GaplessPlaybackSnapshot = {
  chunkIndex: number | null;
  isPlaying: boolean;
  playbackTime: number;
  playbackDuration: number;
  playbackProgress: number;
};

type GaplessPlayerCallbacks = {
  onSnapshot: (snapshot: GaplessPlaybackSnapshot) => void;
  onChunkStart: (chunkIndex: number) => void;
  onCompleted: (chunkIndex: number) => void;
  onError: (error: unknown, chunkIndex: number) => void;
};

type ScheduledChunk = {
  chunk: GaplessTtsChunk;
  prepared: PreparedTtsPlaybackBuffer;
  source: AudioBufferSourceNode;
  startTime: number;
  endTime: number;
  sourceOffset: number;
  pauseOffset: number;
};

type PlaybackPosition = Omit<GaplessPlaybackSnapshot, "chunkIndex"> & {
  chunkIndex: number;
  sourceOffset: number;
  pauseOffset: number;
};

const SCHEDULE_LEAD_SECONDS = 0.02;
// Twelve 250 ms cloud chunks provide up to three seconds of jitter tolerance
// without constructing an unbounded graph for long selections.
const MAX_SCHEDULED_CHUNKS = 12;
const SNAPSHOT_INTERVAL_MS = 50;

/**
 * Provider-independent streaming playback queue.
 *
 * Soniox and Deepgram's official browser examples convert incoming PCM to
 * AudioBuffers scheduled on one AudioContext timeline; OpenAI's cookbook uses
 * the same continuous-context queue principle with an AudioWorklet. This class
 * applies that model to generated chunk files while retaining pause, seek,
 * rate, cancellation, and optional playback effects.
 */
export class GaplessTtsPlayer {
  private callbacks: GaplessPlayerCallbacks;
  private context: AudioContext | null = null;
  private chunks: GaplessTtsChunk[] = [];
  private decoded = new Map<number, PreparedTtsPlaybackBuffer>();
  private scheduled: ScheduledChunk[] = [];
  private expectedTotal = 0;
  private pitch = 1;
  private effect: TtsPlaybackEffect = "none";
  private playbackRate = 1;
  private nextChunkIndex: number | null = null;
  private nextPlayTime = 0;
  private pendingSourceOffset = 0;
  private pendingPauseOffset = 0;
  private desiredPlaying = false;
  private scheduling = false;
  private scheduleRequested = false;
  private generation = 0;
  private decodeAbort: AbortController | null = null;
  private animationFrame: number | null = null;
  private lastSnapshotAt = 0;
  private lastStartedChunk: number | null = null;
  private completionReported = false;

  constructor(callbacks: GaplessPlayerCallbacks) {
    this.callbacks = callbacks;
  }

  setCallbacks(callbacks: GaplessPlayerCallbacks) {
    this.callbacks = callbacks;
  }

  configure(
    pitch: number,
    effect: TtsPlaybackEffect,
    playbackRate: number,
  ) {
    this.pitch = pitch;
    this.effect = effect;
    this.playbackRate = playbackRate;
  }

  setChunks(chunks: GaplessTtsChunk[], expectedTotal: number) {
    const previousPauses = new Map(
      this.chunks.map((chunk) => [chunk.index, chunk.pauseAfterMs]),
    );
    this.chunks = [...chunks].sort((a, b) => a.index - b.index);
    this.expectedTotal = Math.max(0, expectedTotal);

    if (this.nextChunkIndex === null && this.scheduled.length === 0) {
      this.nextChunkIndex = this.chunks[0]?.index ?? null;
    }

    for (const chunk of this.chunks) {
      const previousPause = previousPauses.get(chunk.index);
      if (
        previousPause !== undefined &&
        previousPause !== chunk.pauseAfterMs
      ) {
        this.updateScheduledPause(chunk);
      }
    }

    if (this.desiredPlaying) {
      void this.scheduleAvailable();
    }
  }

  async play() {
    this.desiredPlaying = true;
    this.completionReported = false;
    const context = this.ensureContext();
    if (context.state === "suspended") {
      await context.resume();
    }
    if (this.nextChunkIndex === null && this.scheduled.length === 0) {
      this.nextChunkIndex = this.chunks[0]?.index ?? null;
    }
    this.startMonitor();
    await this.scheduleAvailable();
  }

  pause() {
    this.desiredPlaying = false;
    this.stopMonitor();
    const context = this.context;
    void context
      ?.suspend()
      .then(() => {
        // A fast double-click can request Play while suspend() is pending.
        if (this.desiredPlaying && this.context === context) {
          void context.resume().catch(() => undefined);
        }
      })
      .catch(() => undefined);
    const position = this.currentPosition();
    this.callbacks.onSnapshot({
      chunkIndex: position?.chunkIndex ?? null,
      isPlaying: false,
      playbackTime: position?.playbackTime ?? 0,
      playbackDuration: position?.playbackDuration ?? 0,
      playbackProgress: position?.playbackProgress ?? 0,
    });
  }

  stop() {
    this.desiredPlaying = false;
    this.generation += 1;
    this.decodeAbort?.abort();
    this.decodeAbort = null;
    this.stopScheduledSources();
    this.chunks = [];
    this.decoded.clear();
    this.expectedTotal = 0;
    this.nextChunkIndex = null;
    this.nextPlayTime = 0;
    this.pendingSourceOffset = 0;
    this.pendingPauseOffset = 0;
    this.lastStartedChunk = null;
    this.completionReported = false;
    this.scheduleRequested = false;
    this.stopMonitor();
    const context = this.context;
    this.context = null;
    if (context && context.state !== "closed") {
      void context.close().catch(() => undefined);
    }
    this.callbacks.onSnapshot({
      chunkIndex: null,
      isPlaying: false,
      playbackTime: 0,
      playbackDuration: 0,
      playbackProgress: 0,
    });
  }

  seek(playbackProgress: number): number | null {
    const target = this.resolveSeekTarget(playbackProgress);
    if (!target) {
      return null;
    }
    this.restartFrom(target.chunkIndex, target.sourceOffset);
    return target.playbackProgress;
  }

  setPlaybackRate(playbackRate: number) {
    const position = this.currentPosition();
    this.playbackRate = playbackRate;
    if (!position) {
      return;
    }
    this.restartFrom(
      position.chunkIndex,
      position.sourceOffset,
      position.pauseOffset,
    );
  }

  private ensureContext() {
    if (!this.context || this.context.state === "closed") {
      this.context = new AudioContext({
        latencyHint: "interactive",
      });
      this.nextPlayTime = this.context.currentTime;
    }
    return this.context;
  }

  private async scheduleAvailable() {
    if (!this.desiredPlaying) {
      return;
    }
    if (this.scheduling) {
      this.scheduleRequested = true;
      return;
    }
    this.scheduling = true;
    this.scheduleRequested = false;
    const generation = this.generation;
    try {
      const context = this.ensureContext();
      while (
        this.desiredPlaying &&
        generation === this.generation &&
        this.scheduled.filter((entry) => entry.endTime > context.currentTime)
          .length < MAX_SCHEDULED_CHUNKS
      ) {
        const chunk = this.chunks.find(
          (candidate) => candidate.index === this.nextChunkIndex,
        );
        if (!chunk) {
          break;
        }

        let prepared = this.decoded.get(chunk.index);
        if (!prepared) {
          const controller = new AbortController();
          this.decodeAbort = controller;
          try {
            prepared = await prepareTtsPlaybackBuffer(
              convertFileSrc(chunk.path, "asset"),
              this.pitch,
              this.effect,
              context,
              controller.signal,
            );
          } catch (error) {
            if (!controller.signal.aborted && generation === this.generation) {
              this.desiredPlaying = false;
              this.callbacks.onError(error, chunk.index);
            }
            return;
          } finally {
            if (this.decodeAbort === controller) {
              this.decodeAbort = null;
            }
          }
          if (generation !== this.generation || !this.desiredPlaying) {
            return;
          }
          this.decoded.set(chunk.index, prepared);
        }

        const source = context.createBufferSource();
        source.buffer = prepared.buffer;
        const effectivePlaybackRate =
          this.playbackRate / prepared.pitchCompensation;
        source.playbackRate.value = effectivePlaybackRate;
        source.connect(context.destination);
        const sourceOffset = Math.min(
          prepared.buffer.duration,
          Math.max(0, this.pendingSourceOffset),
        );
        this.pendingSourceOffset = 0;
        const pauseDuration = Math.max(0, chunk.pauseAfterMs) / 1_000;
        const pauseOffset = Math.min(
          pauseDuration,
          Math.max(0, this.pendingPauseOffset),
        );
        this.pendingPauseOffset = 0;
        const startTime = Math.max(
          context.currentTime + SCHEDULE_LEAD_SECONDS,
          this.nextPlayTime,
        );
        const endTime =
          startTime +
          (prepared.buffer.duration - sourceOffset) / effectivePlaybackRate;
        const entry: ScheduledChunk = {
          chunk,
          prepared,
          source,
          startTime,
          endTime,
          sourceOffset,
          pauseOffset,
        };
        this.scheduled.push(entry);
        this.nextPlayTime = endTime + Math.max(0, pauseDuration - pauseOffset);
        this.nextChunkIndex = chunk.index + 1;
        source.onended = () => {
          if (generation !== this.generation) {
            return;
          }
          void this.scheduleAvailable();
          this.maybeReportCompletion();
        };
        source.start(startTime, sourceOffset);
      }
    } finally {
      this.scheduling = false;
      if (this.scheduleRequested && this.desiredPlaying) {
        this.scheduleRequested = false;
        void this.scheduleAvailable();
      }
    }
  }

  private updateScheduledPause(chunk: GaplessTtsChunk) {
    const entryIndex = this.scheduled.findIndex(
      (entry) => entry.chunk.index === chunk.index,
    );
    if (entryIndex < 0) {
      return;
    }
    const entry = this.scheduled[entryIndex];
    entry.chunk = chunk;
    const pauseDuration = Math.max(0, chunk.pauseAfterMs) / 1_000;
    entry.pauseOffset = Math.min(entry.pauseOffset, pauseDuration);
    const future = this.scheduled.splice(entryIndex + 1);
    for (const scheduled of future) {
      scheduled.source.onended = null;
      try {
        scheduled.source.stop();
      } catch {
        // A source that has already ended needs no cleanup.
      }
      scheduled.source.disconnect();
    }
    this.nextChunkIndex = chunk.index + 1;
    this.nextPlayTime =
      entry.endTime + Math.max(0, pauseDuration - entry.pauseOffset);
    if (this.desiredPlaying) {
      void this.scheduleAvailable();
    }
  }

  private restartFrom(
    chunkIndex: number,
    sourceOffset: number,
    pauseOffset = 0,
  ) {
    this.generation += 1;
    this.decodeAbort?.abort();
    this.decodeAbort = null;
    this.stopScheduledSources();
    this.nextChunkIndex = chunkIndex;
    this.pendingSourceOffset = sourceOffset;
    this.pendingPauseOffset = pauseOffset;
    this.nextPlayTime = this.ensureContext().currentTime + SCHEDULE_LEAD_SECONDS;
    this.lastStartedChunk = null;
    this.completionReported = false;
    if (this.desiredPlaying) {
      void this.scheduleAvailable();
    }
  }

  private stopScheduledSources() {
    for (const entry of this.scheduled) {
      entry.source.onended = null;
      try {
        entry.source.stop();
      } catch {
        // A source that has already ended needs no cleanup.
      }
      entry.source.disconnect();
    }
    this.scheduled = [];
  }

  private currentPosition(): PlaybackPosition | null {
    const context = this.context;
    if (!context) {
      return null;
    }
    const now = context.currentTime;
    const active = this.scheduled.find(
      (entry) => now >= entry.startTime && now < entry.endTime,
    );
    if (active) {
      const sourceTime = Math.min(
        active.prepared.buffer.duration,
        active.sourceOffset +
          Math.max(0, now - active.startTime) *
            (this.playbackRate / active.prepared.pitchCompensation),
      );
      return this.positionOnTimeline(active.chunk.index, sourceTime);
    }

    const previous = [...this.scheduled]
      .reverse()
      .find((entry) => now >= entry.endTime);
    if (previous) {
      const pauseOffset = Math.min(
        Math.max(0, previous.chunk.pauseAfterMs) / 1_000,
        previous.pauseOffset + Math.max(0, now - previous.endTime),
      );
      return this.positionOnTimeline(
        previous.chunk.index,
        previous.prepared.buffer.duration,
        pauseOffset,
      );
    }
    const future = this.scheduled.find((entry) => now < entry.startTime);
    return future
      ? this.positionOnTimeline(
          future.chunk.index,
          future.sourceOffset,
          future.pauseOffset,
        )
      : null;
  }

  private positionOnTimeline(
    chunkIndex: number,
    sourceOffset: number,
    pauseOffset = 0,
  ): PlaybackPosition | null {
    let elapsed = 0;
    let targetStart: number | null = null;
    let target: PreparedTtsPlaybackBuffer | null = null;
    let targetPauseDuration = 0;
    for (const chunk of this.chunks) {
      const prepared = this.decoded.get(chunk.index);
      if (!prepared) {
        continue;
      }
      if (chunk.index === chunkIndex) {
        targetStart = elapsed;
        target = prepared;
        targetPauseDuration = Math.max(0, chunk.pauseAfterMs) / 1_000;
      }
      elapsed += prepared.buffer.duration * prepared.pitchCompensation;
      // History audio encodes this pause as silence; streaming playback
      // schedules it separately, so the visible media timeline must add it.
      elapsed += Math.max(0, chunk.pauseAfterMs) / 1_000;
    }
    if (targetStart === null || !target) {
      return null;
    }
    const boundedSourceOffset = Math.min(
      target.buffer.duration,
      Math.max(0, sourceOffset),
    );
    const boundedPauseOffset =
      boundedSourceOffset >= target.buffer.duration
        ? Math.min(Math.max(0, pauseOffset), targetPauseDuration)
        : 0;
    return {
      chunkIndex,
      sourceOffset: boundedSourceOffset,
      pauseOffset: boundedPauseOffset,
      isPlaying: false,
      playbackTime:
        targetStart +
        boundedSourceOffset * target.pitchCompensation +
        boundedPauseOffset,
      playbackDuration: elapsed,
      playbackProgress: this.progressForPosition(
        chunkIndex,
        boundedSourceOffset,
        target.buffer.duration,
      ),
    };
  }

  private progressForPosition(
    chunkIndex: number,
    sourceOffset: number,
    sourceDuration: number,
  ) {
    // Decoded duration grows as streamed chunks arrive. Anchor the visible
    // progress to the fixed chunk plan so adding buffered audio cannot move
    // the seek thumb backwards.
    const totalChunks = this.expectedTotal || this.chunks.length;
    if (totalChunks <= 0) {
      return 0;
    }
    const chunkProgress =
      sourceDuration > 0
        ? Math.min(1, Math.max(0, sourceOffset / sourceDuration))
        : 0;
    return Math.min(
      1,
      Math.max(0, (chunkIndex + chunkProgress) / totalChunks),
    );
  }

  private resolveSeekTarget(playbackProgress: number) {
    const available = this.chunks.flatMap((chunk) => {
      const prepared = this.decoded.get(chunk.index);
      return prepared ? [{ chunk, prepared }] : [];
    });
    if (available.length === 0) {
      return null;
    }
    const totalChunks = this.expectedTotal || this.chunks.length;
    const boundedProgress = Math.min(1, Math.max(0, playbackProgress));
    const plannedPosition = boundedProgress * totalChunks;
    const targetChunkIndex = Math.min(
      totalChunks - 1,
      Math.floor(plannedPosition),
    );
    const exactTarget = available.find(
      ({ chunk }) => chunk.index === targetChunkIndex,
    );
    const target =
      exactTarget ??
      [...available]
        .reverse()
        .find(({ chunk }) => chunk.index < targetChunkIndex) ??
      available[0];
    const targetFraction = exactTarget
      ? targetChunkIndex === totalChunks - 1 && boundedProgress === 1
        ? 1
        : plannedPosition - targetChunkIndex
      : 1;
    const sourceOffset =
      target.prepared.buffer.duration *
      Math.min(1, Math.max(0, targetFraction));
    return {
      chunkIndex: target.chunk.index,
      sourceOffset,
      playbackProgress: this.progressForPosition(
        target.chunk.index,
        sourceOffset,
        target.prepared.buffer.duration,
      ),
    };
  }

  private startMonitor() {
    if (this.animationFrame !== null) {
      return;
    }
    const update = () => {
      this.animationFrame = null;
      const context = this.context;
      const position = this.currentPosition();
      const active = context
        ? this.scheduled.find(
            (entry) =>
              context.currentTime >= entry.startTime &&
              context.currentTime < entry.endTime,
          )
        : null;
      const isPlaying = Boolean(
        active && this.desiredPlaying && context?.state === "running",
      );
      const snapshotAt = performance.now();
      if (snapshotAt - this.lastSnapshotAt >= SNAPSHOT_INTERVAL_MS) {
        this.lastSnapshotAt = snapshotAt;
        this.callbacks.onSnapshot({
          chunkIndex: position?.chunkIndex ?? null,
          isPlaying,
          playbackTime: position?.playbackTime ?? 0,
          playbackDuration: position?.playbackDuration ?? 0,
          playbackProgress: position?.playbackProgress ?? 0,
        });
      }
      if (active && this.lastStartedChunk !== active.chunk.index) {
        this.lastStartedChunk = active.chunk.index;
        this.callbacks.onChunkStart(active.chunk.index);
      }
      this.maybeReportCompletion();
      if (this.context && !this.completionReported) {
        this.animationFrame = requestAnimationFrame(update);
      }
    };
    this.animationFrame = requestAnimationFrame(update);
  }

  private stopMonitor() {
    if (this.animationFrame !== null) {
      cancelAnimationFrame(this.animationFrame);
      this.animationFrame = null;
    }
  }

  private maybeReportCompletion() {
    if (
      this.completionReported ||
      this.expectedTotal <= 0 ||
      !this.context
    ) {
      return;
    }
    const last = this.scheduled.find(
      (entry) => entry.chunk.index + 1 === this.expectedTotal,
    );
    if (!last || this.context.currentTime < last.endTime) {
      return;
    }
    this.completionReported = true;
    this.desiredPlaying = false;
    this.stopMonitor();
    void this.context.suspend().catch(() => undefined);
    const finalPosition = this.positionOnTimeline(
      last.chunk.index,
      last.prepared.buffer.duration,
    );
    if (finalPosition) {
      this.callbacks.onSnapshot({
        chunkIndex: finalPosition.chunkIndex,
        isPlaying: false,
        playbackTime: finalPosition.playbackTime,
        playbackDuration: finalPosition.playbackDuration,
        playbackProgress: finalPosition.playbackProgress,
      });
    }
    this.callbacks.onCompleted(last.chunk.index);
  }
}
