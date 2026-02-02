import { create } from "zustand";
import { SidebarSection } from "../components/Sidebar";

interface NavigationState {
  currentSection: SidebarSection;
  setSection: (section: SidebarSection) => void;
}

export const useNavigationStore = create<NavigationState>((set) => ({
  currentSection: "general",
  setSection: (section) => set({ currentSection: section }),
}));
