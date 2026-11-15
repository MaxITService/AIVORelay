import { describe, expect, it } from "bun:test";
import {
  getDisplayNameSortOrder,
  getGroupedDisplayNameSortOrder,
  type DisplayNameSortEntry,
} from "./displayNameSorting";

const entry = (
  name: string,
  { pinned = false, group = null }: { pinned?: boolean; group?: string | null } = {},
): DisplayNameSortEntry => ({ name, pinned, group });

describe("getDisplayNameSortOrder", () => {
  it("keeps the original order when sorting is off", () => {
    expect(getDisplayNameSortOrder(["b", "a", "c"], "off")).toEqual([0, 1, 2]);
  });

  it("handles empty and single-item lists", () => {
    expect(getDisplayNameSortOrder([], "asc")).toEqual([]);
    expect(getDisplayNameSortOrder(["only"], "desc")).toEqual([0]);
  });

  it("sorts numbers inside names naturally", () => {
    expect(
      getDisplayNameSortOrder(["Model 10", "Model 2", "Model 1"], "asc"),
    ).toEqual([2, 1, 0]);
  });

  it("ignores case and accents and keeps ties in original order", () => {
    expect(
      getDisplayNameSortOrder(["beta", "Alpha", "alpha", "Álpha"], "asc"),
    ).toEqual([1, 2, 3, 0]);
  });

  it("reverses names for descending order without reversing ties", () => {
    expect(getDisplayNameSortOrder(["b", "A", "a", "c"], "desc")).toEqual([
      3, 0, 1, 2,
    ]);
  });

  it("does not mutate the input names", () => {
    const names = ["b", "a"];
    getDisplayNameSortOrder(names, "asc");
    expect(names).toEqual(["b", "a"]);
  });
});

describe("getGroupedDisplayNameSortOrder", () => {
  it("keeps every entry in place when sorting is off", () => {
    const entries = [entry("Zeta"), entry("Default", { pinned: true })];
    expect(getGroupedDisplayNameSortOrder(entries, "off")).toEqual([0, 1]);
  });

  it("keeps pinned entries first in caller order in both directions", () => {
    const entries = [
      entry("Zeta"),
      entry("Default", { pinned: true }),
      entry("Alpha"),
      entry("Auto", { pinned: true }),
    ];
    expect(getGroupedDisplayNameSortOrder(entries, "asc")).toEqual([1, 3, 2, 0]);
    expect(getGroupedDisplayNameSortOrder(entries, "desc")).toEqual([1, 3, 0, 2]);
  });

  it("keeps caller group order and sorts names within each group", () => {
    const entries = [
      entry("b", { group: "On device" }),
      entry("z", { group: "Cloud" }),
      entry("a", { group: "On device" }),
      entry("c", { group: "Cloud" }),
    ];
    expect(getGroupedDisplayNameSortOrder(entries, "asc")).toEqual([2, 0, 3, 1]);
    expect(getGroupedDisplayNameSortOrder(entries, "desc")).toEqual([0, 2, 1, 3]);
  });

  it("treats ungrouped entries as a group at their first position", () => {
    const entries = [
      entry("b"),
      entry("x", { group: "Voices" }),
      entry("a"),
    ];
    expect(getGroupedDisplayNameSortOrder(entries, "asc")).toEqual([2, 0, 1]);
  });
});
