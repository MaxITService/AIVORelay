type InitializationResult = { status: "ok"; data: unknown } | { status: "error"; error: string };

interface InputCommands {
  initializeEnigo(): Promise<InitializationResult>;
  initializeShortcuts(): Promise<InitializationResult>;
}

/** Permission being granted does not guarantee that the input backend started. */
export async function initializeInputSystem(commands: InputCommands): Promise<void> {
  const input = await commands.initializeEnigo();
  if (input.status === "error") throw new Error(input.error);
  const shortcuts = await commands.initializeShortcuts();
  if (shortcuts.status === "error") throw new Error(shortcuts.error);
}
