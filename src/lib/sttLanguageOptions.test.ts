import { expect, it } from "bun:test";
import type { ModelInfo } from "@/bindings";
import { sttLanguageOptions } from "./sttLanguageOptions";
import type { SttModelSelection } from "./sttModelSelection";

const remote = (
  provider_preset: string,
  model_id: string,
): SttModelSelection => ({
  provider: "remote_openai_compatible",
  provider_preset,
  model_id,
});

const localModel = {
  id: "local-test",
  supported_languages: ["en", "nb", "zh"],
} as unknown as ModelInfo;

const values = (selection: SttModelSelection, models: ModelInfo[] = []) =>
  sttLanguageOptions(selection, models).map((language) => language.value);

it("scopes Rumble-2 language options to the canonical provider and model", () => {
  const options = values(remote("reson8", "rumble-2"));

  expect(options).toContain("auto");
  expect(options).toContain("os_input");
  expect(options).toContain("no");
  expect(options).toContain("nb");
  expect(options).toContain("tl");
  expect(options).toContain("fil");
  expect(options).toContain("jw");
  expect(options).toContain("zh");
  expect(options).toContain("ceb");
  expect(options).toContain("fy");
  expect(options).not.toContain("he");
  expect(options).not.toContain("la");
  expect(options).not.toContain("haw");

  expect(values(remote("custom", "rumble-2"))).toContain("he");
  expect(values(remote("openai", "rumble-2"))).toContain("he");
  expect(values(remote("reson8", "rumble-2-flash"))).toContain("he");
});

it("keeps Soniox filtering and local-model capability filtering unchanged", () => {
  const soniox = values({
    provider: "remote_soniox",
    provider_preset: "",
    model_id: "stt-rt-v6",
  });
  expect(soniox).toContain("auto");
  expect(soniox).toContain("he");
  expect(soniox).not.toContain("la");
  expect(soniox).not.toContain("haw");

  const local = values(
    {
      provider: "local",
      provider_preset: "",
      model_id: "local-test",
    },
    [localModel],
  );
  expect(local).toContain("auto");
  expect(local).toContain("os_input");
  expect(local).toContain("en");
  expect(local).toContain("no");
  expect(local).toContain("nb");
  expect(local).toContain("zh");
  expect(local).not.toContain("fr");
});
