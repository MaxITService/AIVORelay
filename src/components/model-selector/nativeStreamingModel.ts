import type { ModelInfo } from "@/bindings";

type StreamingModelIdentity = Pick<ModelInfo, "id" | "name" | "filename">;

export function isMoonshineStreamingModel(
  model: StreamingModelIdentity | null | undefined,
): boolean {
  if (!model) {
    return false;
  }

  const hint = `${model.id} ${model.name} ${model.filename}`.toLowerCase();
  return hint.includes("moonshine") && hint.includes("streaming");
}
