import { useCallback, useEffect, useState, type Dispatch, type SetStateAction } from "react";

// UI state survives section changes, but is discarded with this window.
const windowState = new Map<string, unknown>();
const listeners = new Map<string, Set<(value: unknown) => void>>();

// Seeds on first read so a key that changes after mount never yields undefined.
const readWindowState = <T,>(key: string, initialValue: T | (() => T)): T => {
  if (windowState.has(key)) return windowState.get(key) as T;
  const initial = typeof initialValue === "function"
    ? (initialValue as () => T)()
    : initialValue;
  windowState.set(key, initial);
  return initial;
};

export const useWindowState = <T,>(
  key: string,
  initialValue: T | (() => T),
): [T, Dispatch<SetStateAction<T>>] => {
  const [value, setValue] = useState<T>(() => readWindowState(key, initialValue));
  useEffect(() => {
    const listener = (next: unknown) => setValue(next as T);
    const subscribers = listeners.get(key) ?? new Set<(value: unknown) => void>();
    subscribers.add(listener);
    listeners.set(key, subscribers);
    setValue(readWindowState(key, initialValue));
    return () => {
      subscribers.delete(listener);
      if (subscribers.size === 0) listeners.delete(key);
    };
  }, [key]);
  const updateValue = useCallback<Dispatch<SetStateAction<T>>>((nextValue) => {
    const current = windowState.get(key) as T;
    const next = typeof nextValue === "function"
      ? (nextValue as (previous: T) => T)(current)
      : nextValue;
    windowState.set(key, next);
    setValue(next);
    listeners.get(key)?.forEach((listener) => listener(next));
  }, [key]);

  return [value, updateValue];
};
