import catalog from "../src-tauri/src/catalog/catalog.json";
import {
  MODEL_CAPABILITY_LANGUAGES,
  supportsLanguageCode,
} from "../src/lib/constants/languages.ts";

interface CatalogModel {
  name: string;
  languages?: string[];
}

const failures: string[] = [];
const pickerCodes = MODEL_CAPABILITY_LANGUAGES.map((language) => language.value);

// These model-code variants intentionally share one recognition intent. Keep
// the assertion beside catalog coverage so model-family switches cannot
// silently break a user's persisted language choice.
for (const [left, right] of [
  ["nb", "no"],
  ["fil", "tl"],
]) {
  if (
    !supportsLanguageCode([left], right) ||
    !supportsLanguageCode([right], left)
  ) {
    failures.push(`language aliases ${left} and ${right} must remain equivalent`);
  }
}

for (const model of catalog.models as CatalogModel[]) {
  for (const modelLanguage of model.languages ?? []) {
    if (!supportsLanguageCode(pickerCodes, modelLanguage)) {
      failures.push(
        `${model.name}: ${modelLanguage} has no compatible frontend language choice`,
      );
    }
  }
}

if (failures.length > 0) {
  console.error("Model language coverage check failed:\n");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  `Model language coverage: all catalog codes map to one or more of ${MODEL_CAPABILITY_LANGUAGES.length} frontend choices`,
);
