import { catalog, modelFile, type CatalogModel } from "@handy-computer/models";

type AvailableModel = { id: string; is_custom: boolean; is_recommended: boolean };
export type RecommendationRole = "best" | "fast" | "accurate" | "default";

/** Match only the benchmarked default quant, never a custom or alternate file. */
export function catalogModelForAvailable(model: AvailableModel): CatalogModel | undefined {
  if (model.is_custom) return undefined;
  return catalog.curatedModels.find((entry) => entry.role === "asr" && `${entry.repository}/${modelFile(entry).filename}` === model.id);
}

export function recommendedModels(models: readonly AvailableModel[], languages: readonly string[]): Map<string, RecommendationRole> {
  if (!languages.length) return new Map(models.filter((model) => model.is_recommended).map((model) => [model.id, "default"]));
  const available = models.flatMap((model) => {
    const entry = catalogModelForAvailable(model);
    return entry ? [{ info: model, entry }] : [];
  });
  const byCatalogId = new Map(available.map(({ info, entry }) => [entry.id, info.id]));
  const result = new Map<string, RecommendationRole>();
  for (const pick of catalog.recommendModels(available.map(({ entry }) => entry), languages)) {
    const id = byCatalogId.get(pick.model.id);
    const role = pick.roles.includes("best") ? "best" : pick.roles.includes("fast") ? "fast" : pick.roles.includes("accurate") ? "accurate" : null;
    if (id && role) result.set(id, role);
  }
  return result;
}
