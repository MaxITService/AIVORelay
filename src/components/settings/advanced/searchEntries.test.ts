import { describe, expect, it } from "bun:test";
import { advancedSearchEntries } from "./searchEntries";

describe("advanced settings search entries", () => {
  it("enables portable-storage only when context confirms a portable build", () => {
    const portableEntry = advancedSearchEntries.find(
      (entry) => entry.id === "portable-storage",
    );

    expect(portableEntry).toBeDefined();
    expect(portableEntry?.unavailableReasonKey).toBe(
      "settingsSearch.unavailable.portableOnly",
    );
    expect(portableEntry?.unavailableReasonFallback).toBe(
      "Available in the portable version only.",
    );
    expect(portableEntry?.isAvailable?.(null, { isPortable: true })).toBe(true);
    expect(portableEntry?.isAvailable?.(null, { isPortable: false })).toBe(
      false,
    );
    expect(portableEntry?.isAvailable?.(null, { isPortable: null })).toBe(
      false,
    );
    expect(portableEntry?.isAvailable?.(null)).toBe(false);
  });

  it("preserves availability behavior for unrelated advanced entries", () => {
    const modelUnloadEntry = advancedSearchEntries.find(
      (entry) => entry.id === "model-unload",
    );
    const adminAutostartEntry = advancedSearchEntries.find(
      (entry) => entry.id === "admin-autostart",
    );

    expect(modelUnloadEntry).toBeDefined();
    expect(modelUnloadEntry?.isAvailable).toBeUndefined();

    expect(adminAutostartEntry).toBeDefined();
    expect(adminAutostartEntry?.unavailableReasonKey).toBe(
      "settingsSearch.unavailable.windowsOnly",
    );
    expect(typeof adminAutostartEntry?.isAvailable?.(null)).toBe("boolean");
    expect(
      adminAutostartEntry?.isAvailable?.(null, { isPortable: false }),
    ).toBe(adminAutostartEntry?.isAvailable?.(null));
    expect(
      adminAutostartEntry?.isAvailable?.(null, { isPortable: true }),
    ).toBe(adminAutostartEntry?.isAvailable?.(null));
  });
});
