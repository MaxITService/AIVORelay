import { mock } from "bun:test";

// `@/i18n` relies on Vite's import.meta.glob, which bun cannot run. Modules such
// as `activeNotices` import it at load time, so stub it before any test loads.
mock.module("@/i18n", () => ({ default: { t: (key: string) => key } }));
