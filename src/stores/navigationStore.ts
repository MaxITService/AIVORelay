import { create } from "zustand";
import type { SidebarSection } from "../components/Sidebar";

interface NavigationState {
  currentSection: SidebarSection;
  sectionHistory: SidebarSection[];
  sectionHistoryIndex: number;
  pendingHelpAnchor: string | null;
  pendingHelpSearchQuery: string | null;
  setSection: (section: SidebarSection) => void;
  goBack: () => void;
  goForward: () => void;
  openHelp: (anchor?: string) => void;
  openHelpSearch: (query: string) => void;
  consumePendingHelpAnchor: () => string | null;
  consumePendingHelpSearchQuery: () => string | null;
}

const MAX_SECTION_HISTORY = 100;

const pushSection = (state: NavigationState, section: SidebarSection) => {
  if (section === state.currentSection) {
    return {
      currentSection: section,
      sectionHistory: state.sectionHistory,
      sectionHistoryIndex: state.sectionHistoryIndex,
    };
  }

  const history = state.sectionHistory.slice(0, state.sectionHistoryIndex + 1);
  history.push(section);
  const boundedHistory = history.slice(-MAX_SECTION_HISTORY);

  return {
    currentSection: section,
    sectionHistory: boundedHistory,
    sectionHistoryIndex: boundedHistory.length - 1,
  };
};

export const useNavigationStore = create<NavigationState>((set, get) => ({
  currentSection: "general",
  sectionHistory: ["general"],
  sectionHistoryIndex: 0,
  pendingHelpAnchor: null,
  pendingHelpSearchQuery: null,
  setSection: (section) =>
    set((state) => ({
      ...pushSection(state, section),
      pendingHelpAnchor:
        section === "help" ? state.pendingHelpAnchor : null,
      pendingHelpSearchQuery:
        section === "help" ? state.pendingHelpSearchQuery : null,
    })),
  goBack: () =>
    set((state) => {
      if (state.sectionHistoryIndex <= 0) return state;
      const sectionHistoryIndex = state.sectionHistoryIndex - 1;
      return {
        currentSection: state.sectionHistory[sectionHistoryIndex],
        sectionHistoryIndex,
        pendingHelpAnchor: null,
        pendingHelpSearchQuery: null,
      };
    }),
  goForward: () =>
    set((state) => {
      if (state.sectionHistoryIndex >= state.sectionHistory.length - 1) {
        return state;
      }
      const sectionHistoryIndex = state.sectionHistoryIndex + 1;
      return {
        currentSection: state.sectionHistory[sectionHistoryIndex],
        sectionHistoryIndex,
        pendingHelpAnchor: null,
        pendingHelpSearchQuery: null,
      };
    }),
  openHelp: (anchor) =>
    set((state) => ({
      ...pushSection(state, "help"),
      pendingHelpAnchor: anchor ?? null,
      pendingHelpSearchQuery: null,
    })),
  openHelpSearch: (query) =>
    set((state) => ({
      ...pushSection(state, "help"),
      pendingHelpAnchor: null,
      pendingHelpSearchQuery: query,
    })),
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
