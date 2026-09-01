import { afterEach, beforeEach, expect, it } from "bun:test";
import { commands, type AppSettings, type Result } from "@/bindings";
import {
  beginModelDownloadActivationIntent,
  cancelModelDownloadActivationIntent,
  consumeModelDownloadAutoActivation,
  invalidateModelDownloadActivationIntent,
  prepareModelDownloadAutoActivation,
} from "./modelDownloadActivation";

const originalGetCurrentModel = commands.getCurrentModel;
const originalGetAppSettings = commands.getAppSettings;

let mockCurrentModel: () => Promise<Result<string, string>> = async () => ({
  status: "ok",
  data: "base-model",
});

let mockGetAppSettings: () => Promise<Result<AppSettings, string>> = async () => ({
  status: "ok",
  data: { transcription_provider: "local" } as unknown as AppSettings,
});

const setMockSelection = (modelId: string, provider = "local") => {
  mockCurrentModel = async () => ({
    status: "ok",
    data: modelId,
  });
  mockGetAppSettings = async () => ({
    status: "ok",
    data: { transcription_provider: provider } as unknown as AppSettings,
  });
};

beforeEach(() => {
  invalidateModelDownloadActivationIntent();
  setMockSelection("base-model", "local");
  commands.getCurrentModel = () => mockCurrentModel();
  commands.getAppSettings = () => mockGetAppSettings();
});

afterEach(() => {
  invalidateModelDownloadActivationIntent();
  commands.getCurrentModel = originalGetCurrentModel;
  commands.getAppSettings = originalGetAppSettings;
});

it("activates successfully when active model and provider remain unchanged", async () => {
  setMockSelection("whisper-base", "local");
  await beginModelDownloadActivationIntent("whisper-large");

  const token = await prepareModelDownloadAutoActivation("whisper-large");
  expect(typeof token).toBe("number");
  expect(token).not.toBeNull();

  const consumed = consumeModelDownloadAutoActivation("whisper-large", token!);
  expect(consumed).toBe(true);
});

it("blocks intent registration when initial snapshot capture fails", async () => {
  mockCurrentModel = async () => ({
    status: "error",
    error: "model query failed",
  });

  await beginModelDownloadActivationIntent("whisper-large");

  const token = await prepareModelDownloadAutoActivation("whisper-large");
  expect(token).toBeNull();
  expect(consumeModelDownloadAutoActivation("whisper-large", 1)).toBe(false);
});

it("ignores stale overlapping begin calls when superseded by a newer download intent", async () => {
  let resolveSlowModelSnapshot: ((val: Result<string, string>) => void) | null = null;
  mockCurrentModel = () =>
    new Promise((resolve) => {
      resolveSlowModelSnapshot = resolve;
    });

  const slowBeginPromise = beginModelDownloadActivationIntent("slow-model");

  mockCurrentModel = async () => ({ status: "ok", data: "base-model" });
  await beginModelDownloadActivationIntent("fast-model");

  resolveSlowModelSnapshot!({ status: "ok", data: "base-model" });
  await slowBeginPromise;

  const slowToken = await prepareModelDownloadAutoActivation("slow-model");
  expect(slowToken).toBeNull();

  const fastToken = await prepareModelDownloadAutoActivation("fast-model");
  expect(typeof fastToken).toBe("number");
  expect(consumeModelDownloadAutoActivation("fast-model", fastToken!)).toBe(true);
});

it("rejects preparation for a different download ID while preserving the active intent", async () => {
  await beginModelDownloadActivationIntent("target-model");

  const wrongToken = await prepareModelDownloadAutoActivation("unrelated-model");
  expect(wrongToken).toBeNull();

  const validToken = await prepareModelDownloadAutoActivation("target-model");
  expect(typeof validToken).toBe("number");
  expect(consumeModelDownloadAutoActivation("target-model", validToken!)).toBe(true);
});

it("isolates cancellation so unrelated model cancellations do not invalidate active intent", async () => {
  await beginModelDownloadActivationIntent("active-model");

  cancelModelDownloadActivationIntent("other-model");

  const token = await prepareModelDownloadAutoActivation("active-model");
  expect(typeof token).toBe("number");

  cancelModelDownloadActivationIntent("active-model");

  const tokenAfterCancel = await prepareModelDownloadAutoActivation("active-model");
  expect(tokenAfterCancel).toBeNull();
});

it("invalidates auto-activation when user changes model or provider during download", async () => {
  setMockSelection("model-a", "local");
  await beginModelDownloadActivationIntent("model-b");

  setMockSelection("model-switched", "local");

  const token = await prepareModelDownloadAutoActivation("model-b");
  expect(token).toBeNull();

  setMockSelection("model-a", "local");
  await beginModelDownloadActivationIntent("model-c");

  setMockSelection("model-a", "remote_soniox");

  const providerChangedToken =
    await prepareModelDownloadAutoActivation("model-c");
  expect(providerChangedToken).toBeNull();
});

it("aborts activation when state changes while prepare is awaiting model selection snapshot", async () => {
  await beginModelDownloadActivationIntent("target-model");

  let resolvePrepareSnapshot: ((val: Result<string, string>) => void) | null = null;
  mockCurrentModel = () =>
    new Promise((resolve) => {
      resolvePrepareSnapshot = resolve;
    });

  const preparePromise = prepareModelDownloadAutoActivation("target-model");

  invalidateModelDownloadActivationIntent();

  resolvePrepareSnapshot!({ status: "ok", data: "base-model" });
  const token = await preparePromise;

  expect(token).toBeNull();
});

it("enforces one-time token consumption preventing duplicate activation dispatches", async () => {
  await beginModelDownloadActivationIntent("target-model");

  const token = await prepareModelDownloadAutoActivation("target-model");
  expect(token).not.toBeNull();

  expect(consumeModelDownloadAutoActivation("target-model", token!)).toBe(true);
  expect(consumeModelDownloadAutoActivation("target-model", token!)).toBe(false);
  expect(consumeModelDownloadAutoActivation("target-model", token!)).toBe(false);
});

it("rejects mismatched generation token or wrong model ID during consume while keeping intent intact", async () => {
  await beginModelDownloadActivationIntent("target-model");

  const validToken = await prepareModelDownloadAutoActivation("target-model");
  expect(validToken).not.toBeNull();

  expect(consumeModelDownloadAutoActivation("wrong-model", validToken!)).toBe(false);
  expect(consumeModelDownloadAutoActivation("target-model", validToken! + 100)).toBe(false);
  expect(consumeModelDownloadAutoActivation("target-model", validToken! - 1)).toBe(false);

  expect(consumeModelDownloadAutoActivation("target-model", validToken!)).toBe(true);
});
