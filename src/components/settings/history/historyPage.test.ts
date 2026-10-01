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

test("late pagination applies transcript edits and saved events without mutating source rows", () => {
  const previous = [{ ...row(5), text: "Current transcript" }];
  const page = [{ ...row(4), text: "Stale transcript" }];
  const previousSnapshot = previous.map(entry => ({ ...entry }));
  const pageSnapshot = page.map(entry => ({ ...entry }));
  const edited = { ...row(4), text: "Corrected transcript" };

  const result = reconcileHistoryPage(previous, page, false, [
    { action: "updated", entry: edited },
    { action: "toggled", id: 4, saved: true },
  ]);

  expect(result).toEqual([
    { ...row(5), text: "Current transcript" },
    { ...row(4, true), text: "Corrected transcript" },
  ]);
  expect(previous).toEqual(previousSnapshot);
  expect(page).toEqual(pageSnapshot);
  expect(edited).toEqual({ ...row(4), text: "Corrected transcript" });
});

test("first-page reload replaces previously loaded rows with the fetched page", () => {
  expect(reconcileHistoryPage([row(5), row(4)], [row(3), row(2)], true, []))
    .toEqual([row(3), row(2)]);
});

test("empty first-page reload removes old rows while retaining new arrivals", () => {
  expect(reconcileHistoryPage([row(5)], [], true, [
    { action: "added", entry: row(6) },
  ])).toEqual([row(6)]);
});

test("addition already present in a fetched page uses the event's latest content", () => {
  expect(reconcileHistoryPage([], [{ ...row(4), text: "Stale" }], true, [
    { action: "added", entry: { ...row(4, true), text: "Latest" } },
  ])).toEqual([{ ...row(4, true), text: "Latest" }]);
});

test("deletion after an addition removes the newly arrived row", () => {
  expect(reconcileHistoryPage([row(3)], [], false, [
    { action: "added", entry: row(4) },
    { action: "deleted", id: 4 },
  ])).toEqual([row(3)]);
});

test("explicit re-addition after deletion restores the row and accepts subsequent edits", () => {
  expect(reconcileHistoryPage([], [{ ...row(4), text: "Old" }], true, [
    { action: "deleted", id: 4 },
    { action: "added", entry: { ...row(4), text: "Restored" } },
    { action: "updated", entry: { ...row(4), text: "Edited" } },
    { action: "toggled", id: 4, saved: true },
  ])).toEqual([{ ...row(4, true), text: "Edited" }]);
});

test("updates and saved acknowledgements after clear cannot repopulate history", () => {
  expect(reconcileHistoryPage([row(5)], [row(4)], false, [
    { action: "cleared" },
    { action: "updated", entry: row(5, true) },
    { action: "toggled", id: 4, saved: true },
  ])).toEqual([]);
});
