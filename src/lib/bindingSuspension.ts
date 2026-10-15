import { invoke } from "@tauri-apps/api/core";

export type BindingSuspensionCommand = "suspend_all_bindings" | "resume_all_bindings";

/**
 * Keeps the app's global shortcuts unregistered while key-capture fields are
 * recording, so the captured keys do not also run app actions. Commands run
 * one at a time in call order; shortcuts return when the last holder releases.
 */
export function createBindingSuspension(
  run: (command: BindingSuspensionCommand) => Promise<unknown>,
) {
  let holders = 0;
  let queue: Promise<unknown> = Promise.resolve();
  const enqueue = (command: BindingSuspensionCommand) => {
    queue = queue.then(() => run(command)).catch(console.error);
  };
  return {
    /** Returns the release function; calling it again is a no-op. */
    hold(): () => void {
      if (holders++ === 0) enqueue("suspend_all_bindings");
      let released = false;
      return () => {
        if (released) return;
        released = true;
        if (--holders === 0) enqueue("resume_all_bindings");
      };
    },
    settled: (): Promise<unknown> => queue,
  };
}

export const holdBindingSuspension = createBindingSuspension((command) => invoke(command)).hold;
