import type { StateStorage } from "zustand/middleware";

/**
 * `localStorage` for zustand `persist` that never throws. `persist` writes on
 * every `set()` and lets a storage exception (quota exceeded, storage
 * disabled) escape from `set()` itself, which would abort the caller halfway.
 * A failed write only loses persistence; the in-memory state stays valid.
 */
export const safeLocalStorage: StateStorage = {
  getItem: (name) => {
    try {
      return localStorage.getItem(name);
    } catch (error) {
      console.warn(`Could not read "${name}" from localStorage:`, error);
      return null;
    }
  },
  setItem: (name, value) => {
    try {
      localStorage.setItem(name, value);
    } catch (error) {
      console.warn(`Could not persist "${name}" to localStorage:`, error);
    }
  },
  removeItem: (name) => {
    try {
      localStorage.removeItem(name);
    } catch (error) {
      console.warn(`Could not remove "${name}" from localStorage:`, error);
    }
  },
};
