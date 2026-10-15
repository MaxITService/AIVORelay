import { expect, test } from "bun:test";
import { createElement } from "react";
import { renderToString } from "react-dom/server";
import {
  readWindowState,
  subscribeWindowState,
  useWindowState,
  writeWindowState,
} from "./useWindowState";

// Window state is module-wide, so every test uses its own key prefix.

test("a key first read after another key was seeded gets its own initial value", () => {
  // 1.0.48 regression: the global profile's draft key gained the STT model id
  // after mount, the new key read undefined, and `.length` blanked the window.
  readWindowState("seed:draft", "edited prompt");
  expect(readWindowState("seed:draft-model-b", () => "baseline b")).toBe("baseline b");
  expect(readWindowState("seed:draft", "ignored")).toBe("edited prompt");
});

test("the hook renders the value of the key it is rendered with", () => {
  const Probe = ({ stateKey }: { stateKey: string }) => {
    const [draft] = useWindowState(stateKey, () => `baseline for ${stateKey}`);
    return createElement("span", null, `${draft}:${draft.length}`);
  };
  writeWindowState("hook:draft", "edited");

  expect(renderToString(createElement(Probe, { stateKey: "hook:draft" }))).toContain("edited:6");
  expect(renderToString(createElement(Probe, { stateKey: "hook:draft-model-b" })))
    .toContain("baseline for hook:draft-model-b:");
});

test("writes reach only the listeners of their key", () => {
  let a = 0;
  let b = 0;
  const unsubscribeA = subscribeWindowState("notify:a", () => a++);
  const unsubscribeB = subscribeWindowState("notify:b", () => b++);
  readWindowState("notify:a", 1);

  writeWindowState<number>("notify:a", (previous) => previous + 1);
  expect(readWindowState("notify:a", 0)).toBe(2);
  expect([a, b]).toEqual([1, 0]);

  unsubscribeA();
  writeWindowState("notify:a", 5);
  expect(a).toBe(1);
  unsubscribeB();
});
