import { commands } from "@/bindings";
import { sttSelectionKey } from "@/lib/sttModelSelection";

interface ModelSelectionSnapshot {
  modelId: string;
  provider: string;
  profileId: string;
  profileModelSelectionKey: string | null;
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

    const profileId = settingsResult.data.active_profile_id ?? "default";
    const profileSelection = profileId === "default"
      ? null
      : settingsResult.data.transcription_profiles?.find(
          (profile) => profile.id === profileId,
        )?.stt_model_selection_override;
    return {
      modelId: modelResult.data,
      provider: String(settingsResult.data.transcription_provider ?? "local"),
      profileId,
      profileModelSelectionKey: profileSelection ? sttSelectionKey(profileSelection) : null,
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
  expectedProfileId?: string,
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
    currentSelection.provider !== intent.provider ||
    currentSelection.profileId !== intent.profileId ||
    currentSelection.profileModelSelectionKey !== intent.profileModelSelectionKey ||
    (expectedProfileId !== undefined && currentSelection.profileId !== expectedProfileId)
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
