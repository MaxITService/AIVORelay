import { create } from "zustand";
import type { SidebarSection } from "../components/Sidebar";

interface NavigationState {
  currentSection: SidebarSection;
  pendingHelpAnchor: string | null;
  pendingHelpSearchQuery: string | null;
  setSection: (section: SidebarSection) => void;
  openHelp: (anchor?: string) => void;
  openHelpSearch: (query: string) => void;
  consumePendingHelpAnchor: () => string | null;
  consumePendingHelpSearchQuery: () => string | null;
}

export const useNavigationStore = create<NavigationState>((set, get) => ({
  currentSection: "general",
  pendingHelpAnchor: null,
  pendingHelpSearchQuery: null,
  setSection: (section) =>
    set((state) => ({
      currentSection: section,
      pendingHelpAnchor:
        section === "help" ? state.pendingHelpAnchor : null,
      pendingHelpSearchQuery:
        section === "help" ? state.pendingHelpSearchQuery : null,
    })),
  openHelp: (anchor) =>
    set({
      currentSection: "help",
      pendingHelpAnchor: anchor ?? null,
      pendingHelpSearchQuery: null,
    }),
  openHelpSearch: (query) =>
    set({
      currentSection: "help",
      pendingHelpAnchor: null,
      pendingHelpSearchQuery: query,
    }),
  consumePendingHelpAnchor: () => {
    const anchor = get().pendingHelpAnchor;
    if (anchor) set({ pendingHelpAnchor: null });
    return anchor;
  },
  consumePendingHelpSearchQuery: () => {
    const query = get().pendingHelpSearchQuery;
    if (query) set({ pendingHelpSearchQuery: null });
    return query;
  },
}));
