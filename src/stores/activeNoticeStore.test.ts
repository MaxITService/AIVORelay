import { afterEach, describe, expect, test } from "bun:test";
import { useActiveNoticeStore as store, type ActiveNotice } from "./activeNoticeStore";

const initial = store.getState();
afterEach(() => store.setState(initial, true));
const notice = (id: number): ActiveNotice => ({ id, source: "model", level: "error",
  title: "Failed", title_key: null, detail: null, target: "models", generation: id });

describe("active notice reconciliation", () => {
  test("late initial fetch cannot resurrect a dismissed notice", () => {
    expect(store.getState().apply({ revision: 2, notices: [], raised: null })).toBe(true);
    expect(store.getState().apply({ revision: 1, notices: [notice(1)], raised: null })).toBe(false);
    expect(store.getState().notices).toEqual([]);
  });
  test("stale dismissal snapshot cannot replace a new failure", () => {
    store.getState().apply({ revision: 3, notices: [notice(2)], raised: notice(2) });
    expect(store.getState().apply({ revision: 2, notices: [], raised: null })).toBe(false);
    expect(store.getState().notices.map(n => n.id)).toEqual([2]);
    expect(store.getState().apply({ revision: 3, notices: [], raised: null })).toBe(false);
  });
});
