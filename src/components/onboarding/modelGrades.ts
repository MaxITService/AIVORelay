import { catalog, modelMatchesLanguage } from "@handy-computer/models";
import { catalogModelForAvailable } from "./modelRecommendations";

export type LanguageGrade = { language: string; kind: "measured"; label: string } | { language: string; kind: "unmeasured" | "unsupported" };

/** Catalog grades apply only to the exact file used for the published benchmark. */
export function modelLanguageGrades(model: { id: string; is_custom: boolean; is_recommended: boolean }, languages: readonly string[]): LanguageGrade[] {
  const entry = catalogModelForAvailable(model);
  if (!entry) return [];
  return languages.map((language) => {
    if (!modelMatchesLanguage(entry, language)) return { language, kind: "unsupported" };
    const grade = catalog.languageAccuracyGrade(entry, language);
    return grade ? { language, kind: "measured", label: grade.label } : { language, kind: "unmeasured" };
  });
}
