import { scrollAndFocusAnchor } from "@/lib/anchorNavigation";
import { useNavigationStore } from "@/stores/navigationStore";
import { useSettingsStore } from "@/stores/settingsStore";

const PROFILE_ANCHOR_PREFIX = "transcription-profile-";

export const getTranscriptionProfileAnchorId = (profileId: string): string =>
  `${PROFILE_ANCHOR_PREFIX}${encodeURIComponent(profileId)}`;

export const openActiveTranscriptionProfile = (): void => {
  const activeProfileId =
    useSettingsStore.getState().settings?.active_profile_id || "default";
  const anchorId = getTranscriptionProfileAnchorId(activeProfileId);

  useNavigationStore.getState().setSection("general");
  window.history.replaceState(null, "", `#${anchorId}`);

  let attempts = 0;
  const scrollToProfile = () => {
    const profile = document.getElementById(anchorId);

    if (profile) {
      scrollAndFocusAnchor(profile, "center");
      return;
    }

    attempts += 1;
    if (attempts <= 20) {
      window.setTimeout(scrollToProfile, 50);
    }
  };

  window.setTimeout(scrollToProfile, 0);
};
