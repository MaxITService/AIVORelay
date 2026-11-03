import { expect, test } from "bun:test";
import { catalog, modelFile } from "@handy-computer/models";
import { modelLanguageGrades } from "./modelGrades";

const whisper = catalog.curatedModels.find((model) => model.id === "whisper-medium")!;
const file = { id: `${whisper.repository}/${modelFile(whisper).filename}`, is_custom: false, is_recommended: false };

test("grades distinguish measured, unmeasured, and unsupported languages", () => {
  const grades = modelLanguageGrades(file, ["en-US", "ba", "zz"]);
  expect(grades[0].kind).toBe("measured");
  expect(grades[1]).toEqual({ language: "ba", kind: "unmeasured" });
  expect(grades[2]).toEqual({ language: "zz", kind: "unsupported" });
  expect(modelLanguageGrades(file, [])).toEqual([]);
});

test("grades are not assigned to custom files or alternate quants", () => {
  expect(modelLanguageGrades({ ...file, is_custom: true }, ["en"])).toEqual([]);
  expect(modelLanguageGrades({ ...file, id: `${file.id}.other` }, ["en"])).toEqual([]);
});
