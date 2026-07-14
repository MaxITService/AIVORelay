import { commands } from "@/bindings";

interface ModelSelectionSnapshot {
  modelId: string;
  provider: string;
}

interface ModelDownloadActivationIntent extends ModelSelectionSnapshot {
  generation: number;
  modelIdToActivate: string;
}

let latestGeneration = 0;
let pendingIntent: ModelDownloadActivationIntent | null = null;

const captureModelSelection =
  async (): Promise<ModelSelectionSnapshot | null> => {
    const [modelResult, settingsResult] = await Promise.all([
      commands.getCurrentModel(),
      commands.getAppSettings(),
    ]);

    if (modelResult.status !== "ok" || settingsResult.status !== "ok") {
      return null;
    }

    return {
      modelId: modelResult.data,
      provider: String(settingsResult.data.transcription_provider ?? "local"),
    };
  };

/**
 * Record that this download is now the only one allowed to auto-activate.
 * The snapshot prevents a completion from overriding a later manual model or
 * provider selection.
 */
export const beginModelDownloadActivationIntent = async (
  modelId: string,
): Promise<void> => {
  const generation = ++latestGeneration;
  const snapshot = await captureModelSelection();

  if (generation !== latestGeneration) {
    return;
  }

  pendingIntent = snapshot
    ? {
        ...snapshot,
        generation,
        modelIdToActivate: modelId,
      }
    : null;
};

/** Cancel an intent only when it belongs to this failed/cancelled download. */
export const cancelModelDownloadActivationIntent = (modelId: string): void => {
  if (pendingIntent?.modelIdToActivate !== modelId) {
    return;
  }

  latestGeneration += 1;
  pendingIntent = null;
};

/** Any explicit user selection supersedes every pending download activation. */
export const invalidateModelDownloadActivationIntent = (): void => {
  latestGeneration += 1;
  pendingIntent = null;
};

/**
 * Return an activation token only if this is still the latest requested
 * download and the user has not changed the active model/provider since it
 * started.
 */
export const prepareModelDownloadAutoActivation = async (
  modelId: string,
): Promise<number | null> => {
  const intent = pendingIntent;
  if (
    !intent ||
    intent.modelIdToActivate !== modelId ||
    intent.generation !== latestGeneration
  ) {
    return null;
  }

  const currentSelection = await captureModelSelection();
  if (
    pendingIntent?.generation !== intent.generation ||
    intent.generation !== latestGeneration
  ) {
    return null;
  }

  if (
    !currentSelection ||
    currentSelection.modelId !== intent.modelId ||
    currentSelection.provider !== intent.provider
  ) {
    invalidateModelDownloadActivationIntent();
    return null;
  }

  return intent.generation;
};

/** Consume a prepared token immediately before dispatching model activation. */
export const consumeModelDownloadAutoActivation = (
  modelId: string,
  generation: number,
): boolean => {
  if (
    latestGeneration !== generation ||
    pendingIntent?.generation !== generation ||
    pendingIntent.modelIdToActivate !== modelId
  ) {
    return false;
  }

  latestGeneration += 1;
  pendingIntent = null;
  return true;
};
