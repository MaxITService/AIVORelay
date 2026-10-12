import { describe, expect, test } from "bun:test";
import { describeProfileOutputPlan, type ProfileOutputPlanInput } from "./profileOutputPlan";

const base = (overrides: Partial<ProfileOutputPlanInput> = {}): ProfileOutputPlanInput => ({
  isWindows: true, heldInPreview: false, streamsLiveText: false, pasteMethod: "ctrl_v",
  clipboardHandling: "restore_plain_text", checkActiveWindow: false, resumeDelayMs: 500,
  stopAction: "preview", waitLimitSecs: 0, waitLimitAction: "preview", openingKey: "", startKey: "", startDelayMs: 150, openDelayMs: 150,
  cancelKey: "", autoSubmit: false, submitKey: "enter", submitDelayMs: 50,
  labels: { method: "Clipboard (Ctrl+V)", clipboard: "Plain text only", openingKey: "Enter", startKey: "Enter", submitKey: "Enter", cancelKey: "Escape", waitLimit: "5 min" },
  ...overrides,
});
const keys = (items: { key: string }[]) => items.map(item => item.key);

describe("describeProfileOutputPlan", () => {
  test("separate start and completion keys keep their own positions and pauses", () => {
    const input = base({ streamsLiveText: true, startKey: "tab", startDelayMs: 200, openingKey: "enter", openDelayMs: 300 });
    const plan = describeProfileOutputPlan({ ...input, labels: { ...input.labels, startKey: "Tab" } });
    expect(keys(plan.steps)).toContain("stepDuringHeld");
    expect(keys(plan.ifs)).toContain("ifKeysWindowChangeContinues");
    expect(keys(plan.steps)).not.toContain("stepDuringLiveEarly");
    expect(plan.steps.find(step => step.key === "stepStartKeyWindows")?.values).toEqual({ key: "Tab" });
    expect(plan.steps.find(step => step.key === "stepEndWait")?.values).toEqual({ delay: 300 });
    expect(keys(plan.steps).indexOf("stepStartKeyWindows")).toBeLessThan(keys(plan.steps).indexOf("stepEndOpen"));
  });
  test("plain batch insertion lists only insertion and the clipboard result", () => {
    const plan = describeProfileOutputPlan(base());
    expect(keys(plan.steps)).toEqual(["stepInsertInto", "stepEndInsert", "stepEndClipboard"]);
    expect(keys(plan.ifs)).toEqual(["ifPasteFails"]);
    expect(plan.steps[0].values).toEqual({ method: "Clipboard (Ctrl+V)" });
  });

  test("paste method none inserts nothing and presses no keys", () => {
    const plan = describeProfileOutputPlan(base({ pasteMethod: "none", openingKey: "enter", autoSubmit: true }));
    expect(keys(plan.steps)).toEqual(["stepNoInsertion"]);
    expect(plan.ifs).toEqual([]);
  });

  test("key actions around the end insertion keep the open - wait - insert - submit order", () => {
    const plan = describeProfileOutputPlan(base({ openingKey: "enter", autoSubmit: true, openDelayMs: 300, submitDelayMs: 80 }));
    expect(keys(plan.steps)).toEqual([
      "stepInsertInto", "stepEndOpen", "stepEndWait", "stepEndInsert", "stepEndClipboard", "stepEndSubmit",
    ]);
    expect(plan.steps[2].values).toEqual({ delay: 300 });
    expect(plan.steps[5].values).toEqual({ delay: 80, key: "Enter" });
    expect(keys(plan.ifs)).toEqual(["ifCancelNothing", "ifKeysWindowChangeContinues", "ifPasteFails"]);
  });

  test("window protection changes the key-action edge case on Windows only", () => {
    const protectedPlan = describeProfileOutputPlan(base({ openingKey: "enter", checkActiveWindow: true }));
    expect(keys(protectedPlan.ifs)).toContain("ifKeysWindowChangeStops");
    const other = describeProfileOutputPlan(base({ openingKey: "enter", checkActiveWindow: true, isWindows: false }));
    expect(keys(other.ifs)).not.toContain("ifKeysWindowChangeStops");
    expect(keys(other.ifs)).not.toContain("ifKeysWindowChangeContinues");
  });

  test("live text with key actions is held until the end", () => {
    const plan = describeProfileOutputPlan(base({ streamsLiveText: true, openingKey: "enter" }));
    expect(keys(plan.steps)).toEqual([
      "stepInsertInto", "stepDuringHeld", "stepEndOpen", "stepEndWait", "stepEndInsert", "stepEndClipboard",
    ]);
  });

  test("live text without key actions is inserted live and follows the window unless protected", () => {
    const follow = describeProfileOutputPlan(base({ streamsLiveText: true, pasteMethod: "direct" }));
    expect(keys(follow.steps)).toEqual(["stepInsertInto", "stepDuringLive", "stepEndRest"]);
    expect(keys(follow.ifs)).toEqual(["ifSwitchFollow"]);
    const protectedPlan = describeProfileOutputPlan(base({
      streamsLiveText: true, pasteMethod: "direct", checkActiveWindow: true, stopAction: "wait_for_window",
    }));
    expect(keys(protectedPlan.steps)).toContain("stepTargetWindow");
    expect(keys(protectedPlan.ifs)).toEqual(["ifSwitchPause", "ifStop_wait_for_window", "ifWindowGone"]);
    expect(protectedPlan.ifs[0].values).toEqual({ delay: 500 });
  });

  test("a wait limit adds its expiry outcome only when the remainder waits for the window", () => {
    const protectedInput = { streamsLiveText: true, pasteMethod: "direct" as const, checkActiveWindow: true };
    const waits = describeProfileOutputPlan(base({
      ...protectedInput, stopAction: "wait_for_window", waitLimitSecs: 300, waitLimitAction: "history",
    }));
    expect(keys(waits.ifs)).toEqual(["ifSwitchPause", "ifStop_wait_for_window", "ifWaitLimit_history", "ifWindowGone"]);
    expect(waits.ifs[2].values).toEqual({ limit: "5 min" });
    const noLimit = describeProfileOutputPlan(base({ ...protectedInput, stopAction: "wait_for_window" }));
    expect(keys(noLimit.ifs)).not.toContain("ifWaitLimit_preview");
    const preview = describeProfileOutputPlan(base({ ...protectedInput, stopAction: "preview", waitLimitSecs: 300 }));
    expect(keys(preview.ifs).some(key => key.startsWith("ifWaitLimit_"))).toBe(false);
  });

  test("a recording-start key on a live provider inserts live text and keeps only the after key", () => {
    const plan = describeProfileOutputPlan(base({
      streamsLiveText: true, pasteMethod: "direct", startKey: "enter",
      autoSubmit: true, cancelKey: "escape", checkActiveWindow: true,
    }));
    expect(keys(plan.steps)).toEqual([
      "stepInsertInto", "stepTargetWindow", "stepStartKeyWindows", "stepDuringLiveEarly", "stepEndRest", "stepEndSubmit",
    ]);
    expect(keys(plan.ifs)).toEqual([
      "ifEarlyHeld", "ifEarlyFails", "ifSubmitNeedsWindow", "ifCancelKeyWindow",
      "ifSwitchPause", "ifStop_preview", "ifWindowGone",
    ]);
    expect(plan.ifs[3].values).toEqual({ key: "Escape" });
  });

  test("a recording-start key on a batch provider waits out the delay and has no live steps", () => {
    const plan = describeProfileOutputPlan(base({ startKey: "enter", isWindows: false }));
    expect(keys(plan.steps)).toEqual([
      "stepInsertInto", "stepStartKey", "stepEndEarlyWait", "stepEndInsert", "stepEndClipboard",
    ]);
    expect(keys(plan.ifs)).toEqual(["ifEarlyFails", "ifCancelNothingMore", "ifPasteFails"]);
  });

  test("without a recording-start key nothing is pressed at start", () => {
    const withoutKey = describeProfileOutputPlan(base({ autoSubmit: true }));
    expect(keys(withoutKey.steps)).not.toContain("stepStartKey");
    expect(keys(withoutKey.steps)).not.toContain("stepStartKeyWindows");
    expect(keys(withoutKey.ifs)).not.toContain("ifEarlyFails");
  });

  test("Preview holds the text and presses keys only when it is inserted from Preview", () => {
    const plan = describeProfileOutputPlan(base({
      heldInPreview: true, streamsLiveText: true, startKey: "tab", openingKey: "enter", autoSubmit: true,
    }));
    expect(keys(plan.steps)).toEqual([
      "stepPreview", "stepPreviewNoStartKey", "stepFromPreview", "stepEndOpen", "stepEndWait", "stepEndInsert", "stepEndClipboard", "stepEndSubmit",
    ]);
    expect(plan.steps[0].link).toBe("livePreview");
    expect(keys(plan.ifs)).toEqual(["ifKeysWindowChangeContinues"]);
    expect(keys(describeProfileOutputPlan(base({ heldInPreview: true, pasteMethod: "none" })).steps))
      .toEqual(["stepPreview", "stepNoInsertion"]);
  });
});
