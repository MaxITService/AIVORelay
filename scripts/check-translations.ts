import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const LOCALES_DIR = path.join(__dirname, "..", "src", "i18n", "locales");
const REFERENCE_LANG = "en";

type JsonRecord = Record<string, unknown>;

interface ValidationResult {
  valid: boolean;
  missing: string[][];
  extra: string[][];
}

function getLanguageDirs(): string[] {
  const entries = fs.readdirSync(LOCALES_DIR, { withFileTypes: true });
  return entries
    .filter((entry) => entry.isDirectory() && entry.name !== REFERENCE_LANG)
    .map((entry) => entry.name)
    .sort();
}

function loadTranslation(lang: string): JsonRecord | null {
  const filePath = path.join(LOCALES_DIR, lang, "translation.json");
  try {
    const raw = fs.readFileSync(filePath, "utf8");
    return JSON.parse(raw) as JsonRecord;
  } catch (error) {
    console.error(`ERROR: failed to load ${lang}/translation.json`);
    console.error(`  ${(error as Error).message}`);
    return null;
  }
}

function collectLeafPaths(obj: JsonRecord, prefix: string[] = []): string[][] {
  const output: string[][] = [];
  for (const [key, value] of Object.entries(obj)) {
    const nextPath = [...prefix, key];
    if (value !== null && typeof value === "object" && !Array.isArray(value)) {
      output.push(...collectLeafPaths(value as JsonRecord, nextPath));
    } else {
      output.push(nextPath);
    }
  }
  return output;
}

function hasPath(obj: JsonRecord, keyPath: string[]): boolean {
  let current: unknown = obj;
  for (const key of keyPath) {
    if (typeof current !== "object" || current === null) return false;
    const currentObject = current as JsonRecord;
    if (!(key in currentObject)) return false;
    current = currentObject[key];
  }
  return true;
}

function printPathList(title: string, paths: string[][]): void {
  console.log(`  ${title}: ${paths.length}`);
  const sample = paths.slice(0, 15);
  for (const item of sample) {
    console.log(`    - ${item.join(".")}`);
  }
  if (paths.length > sample.length) {
    console.log(`    ... and ${paths.length - sample.length} more`);
  }
}

function main(): void {
  const languages = getLanguageDirs();
  const reference = loadTranslation(REFERENCE_LANG);

  if (!reference) {
    process.exit(1);
  }

  const referencePaths = collectLeafPaths(reference);
  const results = new Map<string, ValidationResult>();
  let hasErrors = false;

  console.log("Translation consistency check");
  console.log(`Reference language: ${REFERENCE_LANG}`);
  console.log(`Reference key count: ${referencePaths.length}`);
  console.log("");

  for (const lang of languages) {
    const data = loadTranslation(lang);
    if (!data) {
      results.set(lang, { valid: false, missing: [], extra: [] });
      hasErrors = true;
      continue;
    }

    const missing = referencePaths.filter((p) => !hasPath(data, p));
    const langPaths = collectLeafPaths(data);
    const extra = langPaths.filter((p) => !hasPath(reference, p));
    const valid = missing.length === 0 && extra.length === 0;

    if (!valid) {
      hasErrors = true;
    }

    results.set(lang, { valid, missing, extra });
  }

  for (const lang of languages) {
    const result = results.get(lang);
    if (!result) continue;

    if (result.valid) {
      console.log(`OK: ${lang}`);
      continue;
    }

    console.log(`FAIL: ${lang}`);
    if (result.missing.length > 0) {
      printPathList("missing keys", result.missing);
    }
    if (result.extra.length > 0) {
      printPathList("extra keys", result.extra);
    }
    console.log("");
  }

  const passed = Array.from(results.values()).filter((r) => r.valid).length;
  const total = languages.length;
  console.log(`Summary: ${passed}/${total} language files passed`);

  process.exit(hasErrors ? 1 : 0);
}

main();
