import type { TFunction } from "i18next";
import { toast } from "sonner";

const SHORTCUT_ENGINE_COMPATIBILITY_PATTERNS = [
  /not compatible with (?:the )?tauri engine/i,
  /invalid shortcut for handykeys/i,
];

type ShortcutEngine = "tauri" | "handy_keys" | "rdev" | string;

const getShortcutEngineName = (engine: ShortcutEngine, t: TFunction) => {
  switch (engine) {
    case "handy_keys":
      return t("settings.debug.shortcutEngine.shortNames.handyKeys");
    case "rdev":
      return t("settings.debug.shortcutEngine.shortNames.rdev");
    case "tauri":
    default:
      return t("settings.debug.shortcutEngine.shortNames.tauri");
  }
};

export const isShortcutEngineCompatibilityError = (error: unknown): boolean => {
  const message = String(error);
  return SHORTCUT_ENGINE_COMPATIBILITY_PATTERNS.some((pattern) =>
    pattern.test(message),
  );
};

export const showShortcutSetErrorToast = (
  error: unknown,
  engine: ShortcutEngine,
  t: TFunction,
) => {
  if (isShortcutEngineCompatibilityError(error)) {
    toast.error(
      t("settings.general.shortcut.errors.unsupportedByEngine", {
        engine: getShortcutEngineName(engine, t),
      }),
    );
    return;
  }

  toast.error(
    t("settings.general.shortcut.errors.set", {
      error: String(error),
    }),
  );
};
