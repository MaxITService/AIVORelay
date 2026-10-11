import { expect, test } from "bun:test";
import {
  availableProfileName,
  getProfileNameError,
  normalizeProfileName,
  profileNameErrorFromBackend,
} from "./profileNames";

const profiles = [
  { id: "a", name: "Работа" },
  { id: "b", name: "Home" },
];

test("normalizeProfileName trims and ignores case", () => {
  expect(normalizeProfileName("  РАБОТА ")).toBe("работа");
});

test("getProfileNameError reports empty names", () => {
  expect(getProfileNameError("", profiles)).toBe("required");
  expect(getProfileNameError("   ", profiles)).toBe("required");
});

test("getProfileNameError reports duplicates regardless of case and edge spaces", () => {
  expect(getProfileNameError("Работа", profiles)).toBe("duplicate");
  expect(getProfileNameError("  работа  ", profiles)).toBe("duplicate");
  expect(getProfileNameError("HOME", profiles)).toBe("duplicate");
});

test("getProfileNameError accepts unique names and a profile's own name", () => {
  expect(getProfileNameError("Работа (2)", profiles)).toBeNull();
  expect(getProfileNameError("Работа", profiles, "a")).toBeNull();
  expect(getProfileNameError("РАБОТА", profiles, "a")).toBeNull();
  expect(getProfileNameError("home", profiles, "a")).toBe("duplicate");
});

test("the built-in Default name is reserved", () => {
  expect(getProfileNameError("Default", profiles)).toBe("duplicate");
  expect(getProfileNameError("  default ", profiles, "a")).toBe("duplicate");
  expect(getProfileNameError("Default (2)", profiles)).toBeNull();
  expect(availableProfileName("Default", profiles)).toBe("Default (2)");
});

test("availableProfileName returns the name unchanged when it is free", () => {
  expect(availableProfileName("  Новый ", profiles)).toBe("Новый");
});

test("availableProfileName picks the first free numeric suffix", () => {
  expect(availableProfileName("Работа", profiles)).toBe("Работа (2)");
  const crowded = [...profiles, { id: "c", name: "Работа (2)" }, { id: "d", name: "работа (3)" }];
  expect(availableProfileName("Работа", crowded)).toBe("Работа (4)");
  expect(availableProfileName("Работа — copy", [...crowded, { id: "e", name: "Работа — copy" }]))
    .toBe("Работа — copy (2)");
});

test("profileNameErrorFromBackend maps backend codes from Error objects and strings", () => {
  expect(profileNameErrorFromBackend(new Error("profile_name_required"))).toBe("required");
  expect(profileNameErrorFromBackend("profile_name_duplicate")).toBe("duplicate");
  expect(profileNameErrorFromBackend(new Error("profile_name_duplicate"))).toBe("duplicate");
});

test("profileNameErrorFromBackend ignores unrelated errors", () => {
  expect(profileNameErrorFromBackend(new Error("Profile not found"))).toBeNull();
  expect(profileNameErrorFromBackend(undefined)).toBeNull();
});
