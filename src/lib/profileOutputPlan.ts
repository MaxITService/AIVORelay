import type { ClipboardHandling, PasteMethod } from "@/bindings";
import { buffersOutputForKeyActions } from "@/lib/postProcessingAvailability";

/** An i18n key under `profileOutput.plan` plus its interpolation values. */
export interface PlanItem {
  key: string;
  values?: Record<string, string | number>;
  /** Appends a clickable link to the settings section that controls this behaviour. */
  link?: "livePreview";
}

export interface ProfileOutputPlan {
  steps: PlanItem[];
  ifs: PlanItem[];
}

export interface ProfileOutputPlanInput {
  isWindows: boolean;
  /** Preview holds the finished text; keys then run when it is inserted from Preview. */
  heldInPreview: boolean;
  /** The speech provider inserts text into the target window while recording. */
  streamsLiveText: boolean;
  pasteMethod: PasteMethod;
  clipboardHandling: ClipboardHandling;
  checkActiveWindow: boolean;
  resumeDelayMs: number;
  stopAction: "history" | "preview" | "wait_for_window";
  /** Seconds a waiting remainder may wait; 0 means no limit. */
  waitLimitSecs: number;
  waitLimitAction: "preview" | "history";
  openingKey: string;
  startKey: string;
  startDelayMs: number;
  openDelayMs: number;
  cancelKey: string;
  autoSubmit: boolean;
  submitKey: string;
  submitDelayMs: number;
  /** Already translated labels of the chosen options. */
  labels: { method: string; clipboard: string; openingKey: string; startKey: string; submitKey: string; cancelKey: string; waitLimit: string };
}

const CLIPBOARD_METHODS: PasteMethod[] = ["ctrl_v", "ctrl_shift_v", "shift_insert"];

/**
 * Describes, in order, what the current output settings do and what happens in the edge
 * cases. Every statement mirrors backend behaviour (clipboard.rs, live_output_buffer.rs,
 * early_open.rs); anything not verified there is left out.
 */
export const describeProfileOutputPlan = (input: ProfileOutputPlanInput): ProfileOutputPlan => {
  const steps: PlanItem[] = [];
  const ifs: PlanItem[] = [];
  const { labels } = input;
  const inserts = input.pasteMethod !== "none";
  const hasOpening = input.openingKey !== "";
  const startDelay = input.startDelayMs;
  const held = input.heldInPreview;

  if (held) {
    steps.push({ key: "stepPreview", link: "livePreview" });
    if (!inserts) {
      steps.push({ key: "stepNoInsertion" });
      return { steps, ifs };
    }
  } else if (!inserts) {
    steps.push({ key: "stepNoInsertion" });
    return { steps, ifs };
  } else {
    steps.push({ key: "stepInsertInto", values: { method: labels.method } });
  }
  if (held && input.startKey !== "") steps.push({ key: "stepPreviewNoStartKey" });

  const earlyOn = !held && input.startKey !== "";
  const buffered = buffersOutputForKeyActions(input.pasteMethod, input.openingKey, input.startKey, input.autoSubmit);
  const liveInserting = !held && input.streamsLiveText && !buffered;
  const liveHeld = !held && input.streamsLiveText && buffered;
  const protection = input.isWindows && input.checkActiveWindow;
  const usesClipboard = CLIPBOARD_METHODS.includes(input.pasteMethod);

  // At recording start.
  if (protection && (earlyOn || liveInserting)) steps.push({ key: "stepTargetWindow" });
  if (earlyOn) {
    steps.push({
      key: input.isWindows ? "stepStartKeyWindows" : "stepStartKey",
      values: { key: labels.startKey },
    });
  }

  // During recording.
  if (liveHeld) steps.push({ key: "stepDuringHeld" });
  if (liveInserting) {
    steps.push(earlyOn
      ? { key: "stepDuringLiveEarly", values: { delay: startDelay, key: labels.startKey } }
      : { key: "stepDuringLive" });
  }

  // At the end (or when inserting from Preview).
  if (held && (hasOpening || input.autoSubmit)) steps.push({ key: "stepFromPreview" });
  if (earlyOn && !liveInserting) steps.push({ key: "stepEndEarlyWait", values: { delay: startDelay, key: labels.startKey } });
  if (hasOpening) {
    steps.push({ key: "stepEndOpen", values: { key: labels.openingKey } });
    steps.push({ key: "stepEndWait", values: { delay: input.openDelayMs } });
  }
  if (liveInserting) {
    steps.push({ key: "stepEndRest" });
  } else if (earlyOn) {
    steps.push({ key: "stepEndInsert", values: { method: labels.method } });
  } else if (held || hasOpening || input.autoSubmit || !input.streamsLiveText) {
    steps.push({ key: "stepEndInsert", values: { method: labels.method } });
  }
  if (usesClipboard) steps.push({ key: "stepEndClipboard", values: { handling: labels.clipboard } });
  if (input.autoSubmit) {
    steps.push({ key: "stepEndSubmit", values: { delay: input.submitDelayMs, key: labels.submitKey } });
  }

  // Edge cases.
  if (earlyOn && input.isWindows) ifs.push({ key: "ifEarlyHeld" });
  if (earlyOn) ifs.push({ key: "ifEarlyFails" });
  if (earlyOn && input.autoSubmit) {
    // Text inserted at the end gets its window outcome below; only live text needs it here.
    ifs.push({ key: protection && liveInserting ? "ifSubmitNeedsWindow" : "ifSubmitNeedsAllText" });
  }
  if (earlyOn) {
    ifs.push(input.cancelKey !== ""
      ? { key: protection ? "ifCancelKeyWindow" : "ifCancelKey", values: { key: labels.cancelKey } }
      : { key: "ifCancelNothingMore" });
  } else if (!held && (hasOpening || input.autoSubmit)) {
    ifs.push({ key: "ifCancelNothing" });
  }

  if (input.isWindows && liveInserting) {
    if (protection) {
      ifs.push({ key: "ifSwitchPause", values: { delay: input.resumeDelayMs } });
      ifs.push({ key: `ifStop_${input.stopAction}`, values: { delay: input.resumeDelayMs } });
      if (input.stopAction === "wait_for_window" && input.waitLimitSecs > 0) {
        ifs.push({ key: `ifWaitLimit_${input.waitLimitAction}`, values: { limit: labels.waitLimit } });
      }
      ifs.push({ key: "ifWindowGone" });
    } else {
      ifs.push({ key: "ifSwitchFollow" });
    }
  }
  // Text inserted at the end goes through the same window check. After a recording-start key
  // the target is the window that received it, so leaving it during recording also stops.
  if (input.isWindows && !liveInserting && (hasOpening || input.autoSubmit || earlyOn)) {
    ifs.push({ key: !protection ? "ifKeysWindowChangeContinues" : earlyOn ? "ifEarlyWindowStops" : "ifKeysWindowChangeStops" });
  }
  if (!liveInserting && !held) ifs.push({ key: "ifPasteFails" });
  return { steps, ifs };
};
