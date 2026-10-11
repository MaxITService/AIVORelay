import { useCallback, useState, type Dispatch, type SetStateAction } from "react";

// UI state survives section changes, but is discarded with this window.
const windowState = new Map<string, unknown>();

export const useWindowState = <T,>(
  key: string,
  initialValue: T | (() => T),
): [T, Dispatch<SetStateAction<T>>] => {
  const [value, setValue] = useState<T>(() => {
    if (windowState.has(key)) return windowState.get(key) as T;
    const initial = typeof initialValue === "function"
      ? (initialValue as () => T)()
      : initialValue;
    windowState.set(key, initial);
    return initial;
  });
  const updateValue = useCallback<Dispatch<SetStateAction<T>>>((nextValue) => {
    setValue((current) => {
      const next = typeof nextValue === "function"
        ? (nextValue as (previous: T) => T)(current)
        : nextValue;
      windowState.set(key, next);
      return next;
    });
  }, [key]);

  return [value, updateValue];
};
