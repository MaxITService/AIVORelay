import { useCallback, useSyncExternalStore, type Dispatch, type SetStateAction } from "react";

// UI state survives section changes, but is discarded with this window.
const windowState = new Map<string, unknown>();
const listeners = new Map<string, Set<() => void>>();

// Seeds on first read, so whatever key a render asks for already has a value.
export const readWindowState = <T,>(key: string, initialValue: T | (() => T)): T => {
  if (windowState.has(key)) return windowState.get(key) as T;
  const initial = typeof initialValue === "function"
    ? (initialValue as () => T)()
    : initialValue;
  windowState.set(key, initial);
  return initial;
};

export const writeWindowState = <T,>(key: string, nextValue: SetStateAction<T>) => {
  const next = typeof nextValue === "function"
    ? (nextValue as (previous: T) => T)(windowState.get(key) as T)
    : nextValue;
  windowState.set(key, next);
  listeners.get(key)?.forEach((listener) => listener());
};

export const subscribeWindowState = (key: string, listener: () => void) => {
  const subscribers = listeners.get(key) ?? new Set<() => void>();
  subscribers.add(listener);
  listeners.set(key, subscribers);
  return () => {
    subscribers.delete(listener);
    if (subscribers.size === 0 && listeners.get(key) === subscribers) listeners.delete(key);
  };
};

// Every render reads the store for its own key, so a key that changes after
// mount can never serve another key's value or a missing one.
export const useWindowState = <T,>(
  key: string,
  initialValue: T | (() => T),
): [T, Dispatch<SetStateAction<T>>] => {
  const subscribe = useCallback(
    (onChange: () => void) => subscribeWindowState(key, onChange),
    [key],
  );
  const getSnapshot = () => readWindowState(key, initialValue);
  const value = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  const setValue = useCallback<Dispatch<SetStateAction<T>>>(
    (nextValue) => writeWindowState(key, nextValue),
    [key],
  );

  return [value, setValue];
};
