export type ProfileNameError = "required" | "duplicate";

type NamedProfile = { id: string; name: string };

export const normalizeProfileName = (name: string): string => name.trim().toLowerCase();

// Mirrors the backend: the built-in profile is not in the custom profile list.
const RESERVED_DEFAULT_PROFILE_NAME = "default";

export const getProfileNameError = (
  name: string,
  profiles: readonly NamedProfile[],
  excludeId?: string,
): ProfileNameError | null => {
  const normalized = normalizeProfileName(name);
  if (!normalized) return "required";
  if (normalized === RESERVED_DEFAULT_PROFILE_NAME) return "duplicate";
  return profiles.some(profile => profile.id !== excludeId && normalizeProfileName(profile.name) === normalized)
    ? "duplicate" : null;
};

export const availableProfileName = (base: string, profiles: readonly NamedProfile[]): string => {
  const trimmed = base.trim();
  if (!getProfileNameError(trimmed, profiles)) return trimmed;
  for (let suffix = 2; ; suffix += 1) {
    const candidate = `${trimmed} (${suffix})`;
    if (!getProfileNameError(candidate, profiles)) return candidate;
  }
};

export const profileNameErrorFromBackend = (error: unknown): ProfileNameError | null => {
  const message = error instanceof Error ? error.message : String(error);
  if (message === "profile_name_required") return "required";
  if (message === "profile_name_duplicate") return "duplicate";
  return null;
};
