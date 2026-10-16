import { useEffect, useState } from "react";
import { commands } from "@/bindings";

/**
 * Whether packaging forces update checks off (AIVORELAY_DISABLE_UPDATER).
 * `null` while the answer is loading; a failed query counts as unlocked.
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
        if (!cancelled) setLocked(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return locked;
};
