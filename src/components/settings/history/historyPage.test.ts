import { expect, test } from "bun:test";
import { reconcileHistoryPage } from "./historyPage";

const row = (id: number, saved = false) => ({ id, saved });

test("late first page retains additions and applies deletion and saved events", () => {
  expect(reconcileHistoryPage([], [row(3), row(2)], true, [
    { action: "added", entry: row(4) },
    { action: "deleted", id: 3 },
    { action: "toggled", id: 2, saved: true },
  ])).toEqual([row(4), row(2, true)]);
});

test("clear during pagination discards stale rows but keeps later additions", () => {
  expect(reconcileHistoryPage([row(5)], [row(4), row(3)], false, [
    { action: "cleared" },
    { action: "added", entry: row(6) },
  ])).toEqual([row(6)]);
});

test("overlapping pages preserve current rows and do not duplicate entries", () => {
  expect(reconcileHistoryPage([row(3, true)], [row(3), row(2)], false, []))
    .toEqual([row(3, true), row(2)]);
});
