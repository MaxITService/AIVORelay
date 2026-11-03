import { expect, test } from "bun:test";
import { catalog, modelFile } from "@handy-computer/models";
import { catalogModelForAvailable, recommendedModels } from "./modelRecommendations";

const offered = catalog.curatedModels.filter((model) => model.role === "asr").map((model) => ({
  id: `${model.repository}/${modelFile(model).filename}`,
  is_custom: false,
  is_recommended: false,
}));

test("recommendations follow languages and only include offered benchmarked files", () => {
  const english = recommendedModels(offered, ["en"]);
  const russian = recommendedModels(offered, ["ru"]);
  expect([...english.values()]).toContain("best");
  expect([...russian.values()]).toContain("best");
  expect([...english.keys()]).not.toEqual([...russian.keys()]);
  expect(recommendedModels(offered, ["zz"]).size).toBe(0);
  expect(recommendedModels([], ["en"]).size).toBe(0);
  expect(recommendedModels(offered.filter((item) => !english.has(item.id)), ["en"]).has([...english.keys()][0])).toBe(false);
});

test("custom files and other quants never inherit a catalog benchmark", () => {
  const first = offered[0];
  expect(catalogModelForAvailable(first)).toBeDefined();
  expect(catalogModelForAvailable({ ...first, is_custom: true })).toBeUndefined();
  expect(catalogModelForAvailable({ ...first, id: `${first.id}.other` })).toBeUndefined();
  expect(recommendedModels([{ ...first, is_recommended: true }], []).get(first.id)).toBe("default");
});
