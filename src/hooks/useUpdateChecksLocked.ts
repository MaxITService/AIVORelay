import { useEffect, useState } from "react";
import { commands } from "@/bindings";

/**
 * Whether packaging forces update checks off (AIVORELAY_DISABLE_UPDATER).
 * Includes edition/package restrictions. Unknown or failed queries stay locked.
 */
export const useUpdateChecksLocked = (): boolean | null => {
  const [locked, setLocked] = useState<boolean | null>(null);

  useEffect(() => {
    let cancelled = false;
    commands
      .isUpdateChecksLocked()
      .then((value) => {
        if (!cancelled) setLocked(value);
      })
      .catch(() => {
        if (!cancelled) setLocked(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return locked;
};
