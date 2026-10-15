import { afterEach, beforeEach, expect, spyOn, test } from "bun:test";
import { createBindingSuspension, type BindingSuspensionCommand } from "./bindingSuspension";

let consoleError: ReturnType<typeof spyOn>;
beforeEach(() => {
  consoleError = spyOn(console, "error").mockImplementation(() => {});
});
afterEach(() => consoleError.mockRestore());

function harness(fail: BindingSuspensionCommand[] = []) {
  const commands: BindingSuspensionCommand[] = [];
  const suspension = createBindingSuspension(async (command) => {
    commands.push(command);
    if (fail.includes(command)) throw new Error(`${command} failed`);
  });
  return { commands, ...suspension };
}

test("a capturing field suspends app shortcuts until it stops capturing", async () => {
  const h = harness();
  const release = h.hold();
  await h.settled();
  expect(h.commands).toEqual(["suspend_all_bindings"]);

  release();
  release();
  await h.settled();
  expect(h.commands).toEqual(["suspend_all_bindings", "resume_all_bindings"]);
});

test("shortcuts stay suspended until the last capturing field releases", async () => {
  const h = harness();
  const first = h.hold();
  const second = h.hold();
  first();
  await h.settled();
  expect(h.commands).toEqual(["suspend_all_bindings"]);

  second();
  await h.settled();
  expect(h.commands).toEqual(["suspend_all_bindings", "resume_all_bindings"]);
});

test("a capture that starts while the previous resume is pending ends suspended", async () => {
  const h = harness();
  h.hold()();
  h.hold();
  await h.settled();
  expect(h.commands).toEqual(["suspend_all_bindings", "resume_all_bindings", "suspend_all_bindings"]);
});

test("a failed suspend still resumes shortcuts on release", async () => {
  const h = harness(["suspend_all_bindings"]);
  h.hold()();
  await h.settled();
  expect(h.commands).toEqual(["suspend_all_bindings", "resume_all_bindings"]);
});
