import { create } from "zustand";
import type { SidebarSection } from "../components/Sidebar";

interface NavigationState {
  currentSection: SidebarSection;
  pendingHelpAnchor: string | null;
  setSection: (section: SidebarSection) => void;
  openHelp: (anchor?: string) => void;
  consumePendingHelpAnchor: () => string | null;
}

export const useNavigationStore = create<NavigationState>((set, get) => ({
  currentSection: "general",
  pendingHelpAnchor: null,
  setSection: (section) =>
    set((state) => ({
      currentSection: section,
      pendingHelpAnchor:
        section === "help" ? state.pendingHelpAnchor : null,
    })),
  openHelp: (anchor) =>
    set({
      currentSection: "help",
      pendingHelpAnchor: anchor ?? null,
    }),
  consumePendingHelpAnchor: () => {
    const anchor = get().pendingHelpAnchor;
    if (anchor) set({ pendingHelpAnchor: null });
    return anchor;
  },
}));
