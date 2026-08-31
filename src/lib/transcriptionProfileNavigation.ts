import { navigateToSettingsAnchor } from "@/lib/anchorNavigation";
import { useNavigationStore } from "@/stores/navigationStore";
import { useSettingsStore } from "@/stores/settingsStore";

const PROFILE_ANCHOR_PREFIX = "transcription-profile-";

export const getTranscriptionProfileAnchorId = (profileId: string): string =>
  `${PROFILE_ANCHOR_PREFIX}${encodeURIComponent(profileId)}`;

export const openActiveTranscriptionProfile = (): void => {
  const activeProfileId =
    useSettingsStore.getState().settings?.active_profile_id || "default";
  const anchorId = getTranscriptionProfileAnchorId(activeProfileId);

  navigateToSettingsAnchor({
    activateSection: () =>
      useNavigationStore.getState().setSection("general"),
    targetId: anchorId,
    block: "center",
  });
};
