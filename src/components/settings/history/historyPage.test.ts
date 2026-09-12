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

test("delete-all ID events preserve a concurrently added row during reload", () => {
  expect(reconcileHistoryPage([], [row(3), row(2)], true, [
    { action: "deleted", id: 2 },
    { action: "added", entry: row(4) },
    { action: "deleted", id: 3 },
  ])).toEqual([row(4)]);
});

test("duplicate saved acknowledgements assign state instead of inverting it twice", () => {
  expect(reconcileHistoryPage([], [row(3)], true, [
    { action: "toggled", id: 3, saved: true },
    { action: "toggled", id: 3, saved: true },
  ])).toEqual([row(3, true)]);
});

test("late updates and saved acknowledgements cannot resurrect a deleted entry", () => {
  expect(reconcileHistoryPage([row(4)], [row(3)], false, [
    { action: "deleted", id: 3 },
    { action: "updated", entry: row(3, true) },
    { action: "toggled", id: 3, saved: true },
  ])).toEqual([row(4)]);
});
