import { expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";

// Raw `invoke` calls bypass the generated bindings, so a renamed command or a
// misspelled argument only fails at runtime (Tauri ignores unknown keys).
const srcDir = join(import.meta.dir, "..");

type Param = { name: string; optional: boolean };

// `<>` count only in types: object literals hold arrow functions.
function splitTopLevel(text: string, open = "<([{", close = ">)]}"): string[] {
  const parts: string[] = [];
  let depth = 0;
  let current = "";
  for (const char of text) {
    if (open.includes(char)) depth++;
    else if (close.includes(char)) depth--;
    if (char === "," && depth === 0) {
      parts.push(current);
      current = "";
    } else current += char;
  }
  if (current.trim()) parts.push(current);
  return parts.map((part) => part.trim()).filter(Boolean);
}

function boundCommands(): Map<string, Param[]> {
  const bindings = readFileSync(join(srcDir, "bindings.ts"), "utf8");
  const commands = new Map<string, Param[]>();
  for (const match of bindings.matchAll(/^async \w+\((.*)\) : Promise<.*\{[\s\S]*?TAURI_INVOKE\("([^"]+)"/gm)) {
    commands.set(match[2], splitTopLevel(match[1]).map((param) => {
      const colon = param.indexOf(":");
      return { name: param.slice(0, colon).trim(), optional: /\|\s*null$/.test(param.slice(colon + 1).trim()) };
    }));
  }
  return commands;
}

/** Top-level keys of the object literal starting at `start`, or null when they can't be read. */
function objectKeys(source: string, start: number): string[] | null {
  let depth = 0;
  let end = start;
  for (; end < source.length; end++) {
    if ("{[(".includes(source[end])) depth++;
    else if ("}])".includes(source[end]) && --depth === 0) break;
  }
  const keys: string[] = [];
  for (const entry of splitTopLevel(source.slice(start + 1, end), "([{", ")]}")) {
    const key = /^["']?([A-Za-z_$][\w$]*)["']?\s*(?::|$)/.exec(entry);
    if (entry.startsWith("...") || !key) return null;
    keys.push(key[1]);
  }
  return keys;
}

function sourceFiles(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return sourceFiles(path);
    return /\.tsx?$/.test(entry.name) && !/\.test\.tsx?$/.test(entry.name) && entry.name !== "bindings.ts" ? [path] : [];
  });
}

type Call = { at: string; command: string; keys: string[] | null };

function rawInvokeCalls(): Call[] {
  const calls: Call[] = [];
  for (const file of sourceFiles(srcDir)) {
    const source = readFileSync(file, "utf8");
    for (const match of source.matchAll(/\binvoke\s*(?:<[^()]*?>)?\s*\(\s*["']([^"']+)["']\s*(,\s*)?/g)) {
      if (match[1].includes(":")) continue; // plugin commands are not in the bindings
      const argsAt = match.index! + match[0].length;
      const keys = !match[2] ? [] : source[argsAt] === "{" ? objectKeys(source, argsAt) : null;
      const line = source.slice(0, match.index).split("\n").length;
      calls.push({ at: `${relative(srcDir, file)}:${line}`, command: match[1], keys });
    }
  }
  return calls;
}

test("raw invoke calls name registered commands with their argument keys", () => {
  const commands = boundCommands();
  const calls = rawInvokeCalls();
  expect(commands.size).toBeGreaterThan(100);
  expect(calls.length).toBeGreaterThan(100);

  const unknownCommands = calls.filter((call) => !commands.has(call.command)).map((call) => `${call.at} ${call.command}`);
  const badArguments = calls.flatMap((call) => {
    const params = commands.get(call.command);
    if (!params || !call.keys) return [];
    const names = new Set(params.map((param) => param.name));
    const unknown = call.keys.filter((key) => !names.has(key));
    const missing = params.filter((param) => !param.optional && !call.keys!.includes(param.name)).map((param) => param.name);
    return unknown.length || missing.length
      ? [`${call.at} ${call.command} unknown [${unknown}] missing [${missing}]`]
      : [];
  });
  expect(unknownCommands).toEqual([]);
  expect(badArguments).toEqual([]);
});
