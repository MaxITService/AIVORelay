import { safeLocalStorage } from "@/lib/safeStorage";

const STORAGE_KEY = "aivorelay.first-start-progress.v1";

export type OnboardingProgress =
  | { step: "select" }
  | { step: "remote" }
  | { step: "remote_ready" }
  | { step: "local"; modelId: string | null };

export function resolveOnboardingProgress(progress: OnboardingProgress | null, needsPermissions: boolean) {
  const mode = needsPermissions ? "permissions" : progress?.step === "local" ? "local" : progress?.step === "remote_ready" ? "welcome" : "select";
  return {
    mode,
    welcomeVariant: progress?.step === "remote_ready" ? "remote" as const : "local" as const,
    showRemoteWizard: !needsPermissions && progress?.step === "remote",
  } as const;
}

export function parseOnboardingProgress(raw: string | null): OnboardingProgress | null {
  if (!raw) return null;
  try {
    const value: unknown = JSON.parse(raw);
    if (!value || typeof value !== "object") return null;
    const record = value as Record<string, unknown>;
    if (record.step === "select" || record.step === "remote" || record.step === "remote_ready") {
      return { step: record.step };
    }
    if (record.step === "local" && (record.modelId === null || (typeof record.modelId === "string" && record.modelId.length > 0))) {
      return { step: "local", modelId: record.modelId };
    }
  } catch {
    // Malformed or inaccessible storage starts the wizard from the beginning.
  }
  return null;
}

export function readOnboardingProgress(): OnboardingProgress | null {
  const raw = safeLocalStorage.getItem(STORAGE_KEY);
  return parseOnboardingProgress(typeof raw === "string" ? raw : null);
}

export function saveOnboardingProgress(progress: OnboardingProgress): void {
  const safe = progress.step === "local"
    ? { step: progress.step, modelId: progress.modelId }
    : { step: progress.step };
  safeLocalStorage.setItem(STORAGE_KEY, JSON.stringify(safe));
}

export function clearOnboardingProgress(): void {
  safeLocalStorage.removeItem(STORAGE_KEY);
}
