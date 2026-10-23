import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import type { FileTranscriptionRequest } from "./transcribeFileStore";
import type { BatchFilePlan, BatchProgress, BatchScanResult, BatchSummary } from "./ttsBatchQueueStore";

// Use real stores and persistence with a small IPC boundary. No module mocks:
// other frontend tests keep their real bindings and Tauri imports.
const originalStorage = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
const storage = new Map<string, string>();
Object.defineProperty(globalThis, "localStorage", {
  configurable: true,
  value: {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => { storage.set(key, value); },
    removeItem: (key: string) => { storage.delete(key); },
  },
});
const { useTranscribeBatchStore: stt } = await import("./transcribeBatchStore");
const { useTranscribeFileStore: single } = await import("./transcribeFileStore");
const { ensureTtsBatchListener, entryKey, queuedRow, useTtsBatchQueueStore: tts } = await import("./ttsBatchQueueStore");
if (originalStorage) Object.defineProperty(globalThis, "localStorage", originalStorage);
else Reflect.deleteProperty(globalThis, "localStorage");

const originalWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
const initialStt = stt.getState();
const initialTts = tts.getState();
const initialSingle = single.getState();
let backend: (command: string, args: any) => Promise<any>;
let progressHandler: (event: { payload: BatchProgress }) => void;
let rejectListener = false;
let calls: Array<{ command: string; args: any }>;
const bitrates = { mp3Bitrate: 128, opusBitrate: 64 };

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((ok, fail) => { resolve = ok; reject = fail; });
  return { promise, resolve, reject };
}

async function until(condition: () => boolean) {
  for (let turn = 0; turn < 100; turn += 1) {
    if (condition()) return;
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
  }
  throw new Error("Queue did not reach the expected state");
}

function request(filePath: string): FileTranscriptionRequest {
  return {
    filePath, profileId: null, saveToFile: false, outputFormat: "text", modelOverride: null,
    customWordsEnabledOverride: true, sonioxOptionsOverride: null, deepgramOptionsOverride: null,
    retryableRemoteApi: false,
  };
}

function plan(name: string, output = `${name}.mp3`): BatchFilePlan {
  return { inputPath: `C:/in/${name}.txt`, relativePath: `${name}.txt`, outputPath: `C:/out/${output}`, sourceCharacters: 10 };
}

function scan(...files: BatchFilePlan[]): BatchScanResult {
  return { outputDirectory: "C:/out", outputFormat: "mp3", recursive: false, files, eligibleCount: files.length, warnings: [] };
}

function folderScan(inputDirectory: string, recursive: boolean, ...files: BatchFilePlan[]): BatchScanResult {
  return { ...scan(...files), inputDirectory, recursive };
}

function nested(name: string): BatchFilePlan {
  return { inputPath: `C:/in/sub/${name}.txt`, relativePath: `sub\\${name}.txt`, outputPath: `C:/out/sub/${name}.mp3`, sourceCharacters: 10 };
}

function batchScans() {
  return calls.filter((call) => call.command === "convert_tts_batch").map((call) => call.args.request.scan as BatchScanResult);
}

function summary(args: any, cancelled = false, statuses?: string[]): BatchSummary {
  const files = args.request.scan.files.map((file: BatchFilePlan, index: number) => ({
    ...queuedRow(file, index), status: statuses?.[index] ?? "completed",
  }));
  return {
    clientId: args.request.clientId, batchId: "batch-1", total: files.length, finished: files.length,
    completed: files.filter((file: any) => file.status === "completed").length,
    failed: files.filter((file: any) => file.status === "failed").length,
    skipped: files.filter((file: any) => file.status === "skipped").length, cancelled, files,
  };
}

function progress(patch: Partial<BatchProgress> = {}): BatchProgress {
  return {
    clientId: tts.getState().activeClientId!, batchId: "batch-1", total: 2, finished: 0,
    completed: 0, skipped: 0, failed: 0, cancelled: false, done: false,
    startedAtMs: 1, file: null, ...patch,
  };
}

function emit(patch: Partial<BatchProgress> = {}) {
  progressHandler({ payload: progress(patch) });
}

function saved(key: string) {
  return JSON.parse(storage.get(key)!).state;
}

beforeEach(() => {
  calls = [];
  rejectListener = false;
  backend = async (command, args) => {
    if (command === "get_file_transcription_recording_state") return { blocksFileTranscription: false };
    if (command === "transcribe_audio_file") return { text: "transcript", saved_file_path: `${args.filePath}.txt` };
    if (command === "convert_tts_batch") return summary(args);
  };
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: {
      setTimeout, dispatchEvent: () => true,
      __TAURI_OS_PLUGIN_INTERNALS__: { os_type: "windows" },
      __TAURI_INTERNALS__: {
        transformCallback: (handler: typeof progressHandler) => { progressHandler = handler; return 1; },
        invoke: async (command: string, args: any) => {
          calls.push({ command, args });
          if (command === "plugin:event|listen") {
            if (rejectListener) throw new Error("Listener unavailable");
            return 1;
          }
          return backend(command, args);
        },
      },
    },
  });
  stt.setState(initialStt, true);
  tts.setState(initialTts, true);
  single.setState(initialSingle, true);
  storage.clear();
});

afterEach(() => {
  stt.setState(initialStt, true);
  tts.setState(initialTts, true);
  single.setState(initialSingle, true);
  if (originalWindow) Object.defineProperty(globalThis, "window", originalWindow);
  else Reflect.deleteProperty(globalThis, "window");
});

describe("TTS batch queue", () => {
  test("a failed listener pauses without losing work and can be registered again", async () => {
    rejectListener = true;
    await tts.getState().beginFromScan(scan(plan("one")), bitrates);
    expect(tts.getState().running).toBe(false);
    expect(tts.getState().error).toContain("Listener unavailable");
    expect(tts.getState().queue).toHaveLength(1);
    expect(calls.some((call) => call.command === "convert_tts_batch")).toBe(false);
    rejectListener = false;
    await tts.getState().resume();
    expect(tts.getState().queue).toHaveLength(0);
  });

  test("the listener is ready before conversion and appended files run in the next batch", async () => {
    const first = deferred<BatchSummary>();
    let firstArgs: any;
    backend = async (command, args) => {
      if (command !== "convert_tts_batch") return;
      expect(progressHandler).toBeDefined();
      if (!firstArgs) { firstArgs = args; return first.promise; }
      return summary(args);
    };
    const run = tts.getState().beginFromScan(scan(plan("one")), bitrates);
    await until(() => !!firstArgs);
    expect(tts.getState().enqueue(scan(plan("two")))).toBe(1);
    await tts.getState().resume(); // A second click must not create another runner.
    first.resolve(summary(firstArgs));
    await run;
    const batches = calls.filter((call) => call.command === "convert_tts_batch");
    expect(batches.map((call) => call.args.request.scan.files.map((file: BatchFilePlan) => file.relativePath))).toEqual([["one.txt"], ["two.txt"]]);
    expect(batches[1].args.request.mp3Bitrate).toBe(128);
    expect(tts.getState().queue).toHaveLength(0);
  });

  test("completed events remove persisted work before the batch promise resolves", async () => {
    const job = deferred<BatchSummary>();
    let args: any;
    backend = async (_command, value) => { args = value; return job.promise; };
    const run = tts.getState().beginFromScan(scan(plan("one"), plan("two")), bitrates);
    await until(() => !!args);
    emit({ file: { ...queuedRow(plan("one"), 0), status: "completed" } });
    expect(saved("aivorelay-tts-batch-queue-v1").queue.map((file: BatchFilePlan) => file.relativePath)).toEqual(["two.txt"]);
    emit({ clientId: "unrelated", file: { ...queuedRow(plan("two"), 1), status: "completed" } });
    expect(tts.getState().queue).toHaveLength(1);
    job.resolve(summary(args));
    await run;
  });

  test("Stop retains the interrupted failed file and unstarted files, but not completed files", async () => {
    const job = deferred<BatchSummary>();
    let args: any;
    backend = async (command, value) => {
      if (command === "convert_tts_batch") { args = value; return job.promise; }
      if (command === "cancel_tts_batch") return true;
    };
    const files = [plan("done"), plan("interrupted"), plan("waiting")];
    const run = tts.getState().beginFromScan(scan(...files), bitrates);
    await until(() => !!args);
    emit({ file: { ...queuedRow(files[0], 0), status: "completed" } });
    await tts.getState().stop("No batch ID");
    emit({ cancelled: true, file: { ...queuedRow(files[1], 1), status: "failed", error: "Cancelled" } });
    job.resolve(summary(args, true, ["completed", "failed", "skipped"]));
    await run;
    expect(tts.getState().queue.map((file) => file.relativePath)).toEqual(["interrupted.txt", "waiting.txt"]);
    expect(tts.getState().running).toBe(false);
    backend = async (_command, value) => summary(value);
    await tts.getState().resume();
    expect(tts.getState().queue).toHaveLength(0);
  });

  test("duplicate input paths are ignored and colliding outputs get distinct suffixes", () => {
    const one = plan("one", "read.mp3");
    tts.setState({ queue: [one] });
    expect(tts.getState().enqueue(scan(
      { ...one, inputPath: "c:\\IN\\ONE.TXT" }, plan("two", "read.mp3"), plan("three", "read.mp3"),
    ))).toBe(2);
    expect(tts.getState().queue.map((file) => file.outputPath)).toEqual(["C:/out/read.mp3", "C:/out/read-2.mp3", "C:/out/read-3.mp3"]);
  });

  test("on Linux, inputs differing only by letter case are distinct files", () => {
    (window as any).__TAURI_OS_PLUGIN_INTERNALS__.os_type = "linux";
    const upper = { inputPath: "/in/Song.txt", relativePath: "Song.txt", outputPath: "/out/Song.mp3", sourceCharacters: 10 };
    const lower = { ...upper, inputPath: "/in/song.txt", relativePath: "song.txt", outputPath: "/out/song.mp3" };
    tts.setState({ completedInputs: ["/in/Song.txt"] });
    expect(tts.getState().enqueue(scan(upper, lower))).toBe(1);
    expect(tts.getState().queue.map((file) => file.inputPath)).toEqual(["/in/song.txt"]);
    tts.setState({ completedInputs: [] });
    expect(tts.getState().enqueue(scan(upper))).toBe(1);
    // Outputs still ignore case, so a case-insensitive volume never gets two writers.
    expect(tts.getState().queue.map((file) => file.outputPath)).toEqual(["/out/song.mp3", "/out/Song-2.mp3"]);
  });

  test("clearing waiting files preserves every file owned by the running backend batch", () => {
    const files = [plan("one"), plan("two"), plan("later")];
    tts.setState({ queue: files, running: true, runningEntries: files.slice(0, 2).map(entryKey) });
    tts.getState().clearWaiting();
    expect(tts.getState().queue).toEqual(files.slice(0, 2));
    tts.setState({ running: false, runningEntries: [] });
    tts.getState().clearWaiting();
    expect(tts.getState().queue).toEqual([]);
  });

  test("rehydration restores only work and output settings and does not start a paid request", async () => {
    tts.setState({
      meta: { ...bitrates, outputDirectory: "C:/out", outputFormat: "mp3" }, queue: [plan("one")],
      completedInputs: ["c:\\in\\done.txt"], running: true,
    });
    const persisted = storage.get("aivorelay-tts-batch-queue-v1")!;
    expect(Object.keys(JSON.parse(persisted).state).sort()).toEqual(["completedInputs", "meta", "queue"]);
    tts.setState(initialTts, true);
    storage.set("aivorelay-tts-batch-queue-v1", persisted);
    await tts.persist.rehydrate();
    expect(tts.getState().queue).toHaveLength(1);
    expect(tts.getState().running).toBe(false);
    expect(calls).toHaveLength(0);
    // A file converted before the restart is not converted again when re-added.
    expect(tts.getState().enqueue(scan(plan("done", "done-2.mp3")))).toBe(0);
  });

  test("adopting a live batch reconciles already completed files and preserves appended work", async () => {
    await ensureTtsBatchListener();
    const one = plan("one"), two = plan("two"), later = plan("later");
    tts.setState({ queue: [one, two, later] });
    tts.getState().attachActiveBatch({
      progress: progress({ clientId: "surviving" }), scan: scan(one, two),
      files: [{ ...queuedRow(one, 0), status: "completed" }],
    }, bitrates);
    expect(tts.getState().queue).toEqual([two, later]);
    expect(tts.getState().completedInputs).toEqual(["c:\\in\\one.txt"]);
    expect(tts.getState().meta?.outputDirectory).toBe("C:/out");
    emit({ cancelled: true, done: true });
    expect(tts.getState().running).toBe(false);
    expect(tts.getState().queue).toEqual([two, later]);
  });

  test("conversion command failures leave all unfinished work paused", async () => {
    backend = async () => { throw new Error("Provider unavailable"); };
    await tts.getState().beginFromScan(scan(plan("one")), bitrates);
    expect(tts.getState().error).toContain("Provider unavailable");
    expect(tts.getState().queue).toHaveLength(1);
    expect(tts.getState().running).toBe(false);
  });

  test("completion events during a reload lookup cannot leave an adopted batch running", async () => {
    await ensureTtsBatchListener();
    const one = plan("one"), two = plan("two");
    tts.setState({ queue: [one, two] });
    const clientId = "finished-during-lookup";
    emit({ clientId, finished: 1, file: { ...queuedRow(one, 0), status: "completed" } });
    emit({ clientId, finished: 2, file: { ...queuedRow(two, 1), status: "completed" } });
    emit({ clientId, finished: 2, completed: 2, done: true });
    tts.getState().attachActiveBatch({
      progress: progress({ clientId }), scan: scan(one, two), files: [],
    }, bitrates);
    expect(tts.getState().running).toBe(false);
    expect(tts.getState().queue).toEqual([]);
    expect(tts.getState().summary?.completed).toBe(2);
    expect(calls.some((call) => call.command === "convert_tts_batch")).toBe(false);
  });

  test("new scans reserve output names from completed files of the active batch", () => {
    const one = plan("one", "read.mp3");
    tts.setState({ queue: [], running: true, runScan: scan(one) });
    expect(tts.getState().enqueue(scan(plan("two", "read.mp3")))).toBe(1);
    expect(tts.getState().queue[0].outputPath).toBe("C:/out/read-2.mp3");
  });

  test("scans receive outputs owned by queued and running files", () => {
    tts.setState({ queue: [plan("two", "b.mp3")], running: true, runScan: scan(plan("one", "a.mp3")) });
    expect(tts.getState().reservedOutputs()).toEqual(["C:/out/b.mp3", "C:/out/a.mp3"]);
  });
});

describe("TTS batch queue scan roots and converted files", () => {
  test("queued files keep their scan root, so nested folder files run in batches of one root", async () => {
    const first = deferred<BatchSummary>();
    let firstArgs: any;
    backend = async (command, args) => {
      if (command !== "convert_tts_batch") return;
      if (!firstArgs) { firstArgs = args; return first.promise; }
      return summary(args);
    };
    const run = tts.getState().beginFromScan(folderScan("C:/in", true, nested("a"), plan("top")), bitrates);
    await until(() => !!firstArgs);
    const other = { ...plan("b"), inputPath: "C:/other/b.txt" };
    expect(tts.getState().enqueue(scan(plan("loose")))).toBe(1);
    expect(tts.getState().enqueue(folderScan("C:/other", false, other))).toBe(1);
    expect(tts.getState().enqueue(folderScan("C:/in", true, nested("c")))).toBe(1);
    first.resolve(summary(firstArgs));
    await run;
    expect(batchScans().map((batch) => [batch.inputDirectory, batch.recursive, batch.files.map((file) => file.relativePath)])).toEqual([
      ["C:/in", true, ["sub\\a.txt", "top.txt"]],
      [null, false, ["loose.txt"]],
      ["C:/other", false, ["b.txt"]],
      ["C:/in", true, ["sub\\c.txt"]],
    ]);
    // The backend receives only the scanned plan, not the queue bookkeeping.
    expect(Object.keys(batchScans()[0].files[0]).sort()).toEqual(["inputPath", "outputPath", "relativePath", "sourceCharacters"]);
    expect(tts.getState().queue).toHaveLength(0);
  });

  test("an adopted recursive batch resumes with the root of its scan", async () => {
    await ensureTtsBatchListener();
    tts.getState().attachActiveBatch({
      progress: progress({ clientId: "surviving" }), scan: folderScan("C:/in", true, nested("a")), files: [],
    }, bitrates);
    emit({ cancelled: true, done: true });
    expect(tts.getState().running).toBe(false);
    await tts.getState().resume();
    expect(batchScans().map((batch) => [batch.inputDirectory, batch.recursive])).toEqual([["C:/in", true]]);
    expect(tts.getState().queue).toHaveLength(0);
  });

  test("re-adding files converted by this queue skips them, while failed files can be added again", async () => {
    const job = deferred<BatchSummary>();
    let args: any;
    backend = async (command, value) => {
      if (command !== "convert_tts_batch") return;
      if (!args) { args = value; return job.promise; }
      return summary(value);
    };
    const run = tts.getState().beginFromScan(scan(plan("done"), plan("failed"), plan("last")), bitrates);
    await until(() => !!args);
    emit({ file: { ...queuedRow(plan("done"), 0), status: "completed" } });
    emit({ file: { ...queuedRow(plan("failed"), 1), status: "failed", error: "Provider error" } });
    // A rescan plans a `-2` output beside the finished result of the same input.
    const rescan = scan(plan("done", "done-2.mp3"), plan("failed"), plan("last"));
    expect(tts.getState().enqueue(rescan)).toBe(1);
    expect(tts.getState().queue.map((file) => file.relativePath)).toEqual(["last.txt", "failed.txt"]);
    job.resolve(summary(args, false, ["completed", "failed", "completed"]));
    await run;
    // "last" finished without a per-file event; the batch result records it.
    expect(saved("aivorelay-tts-batch-queue-v1").completedInputs.sort()).toEqual([
      "c:\\in\\done.txt", "c:\\in\\failed.txt", "c:\\in\\last.txt",
    ]);
    expect(batchScans().map((batch) => batch.files.map((file) => file.relativePath))).toEqual([
      ["done.txt", "failed.txt", "last.txt"], ["failed.txt"],
    ]);
    expect(tts.getState().enqueue(rescan)).toBe(0);
  });

  test("a failed file re-added during its batch is waiting, so clearing waiting files removes it", async () => {
    const job = deferred<BatchSummary>();
    let args: any;
    backend = async (command, value) => {
      if (command !== "convert_tts_batch") return;
      if (!args) { args = value; return job.promise; }
      return summary(value);
    };
    const run = tts.getState().beginFromScan(scan(plan("failed"), plan("last")), bitrates);
    await until(() => !!args);
    emit({ file: { ...queuedRow(plan("failed"), 0), status: "failed", error: "Provider error" } });
    expect(tts.getState().enqueue(scan(plan("failed")))).toBe(1);
    const readded = tts.getState().queue.at(-1)!;
    expect(readded.outputPath).toBe("C:/out/failed-2.mp3");
    expect(tts.getState().runningEntries).not.toContain(entryKey(readded));
    tts.getState().clearWaiting();
    expect(tts.getState().queue.map((file) => file.relativePath)).toEqual(["last.txt"]);
    job.resolve(summary(args, false, ["failed", "completed"]));
    await run;
    expect(batchScans()).toHaveLength(1);
  });

  test("starting a new queue forgets the files converted by the previous queue", async () => {
    await tts.getState().beginFromScan(scan(plan("one")), bitrates);
    expect(tts.getState().completedInputs).toEqual(["c:\\in\\one.txt"]);
    const job = deferred<BatchSummary>();
    let args: any;
    backend = async (_command, value) => { args = value; return job.promise; };
    const run = tts.getState().beginFromScan(scan(plan("one", "one-2.mp3")), bitrates);
    await until(() => !!args);
    expect(tts.getState().completedInputs).toEqual([]);
    job.resolve(summary(args));
    await run;
    expect(batchScans()).toHaveLength(2);
  });
});

describe("file transcription batch queue", () => {
  test("adding files deduplicates Windows paths, snapshots options, and forces Documents output", () => {
    const input = request("C:/audio/one.wav");
    input.sonioxOptionsOverride = { languageHints: ["en"], enableSpeakerDiarization: true, enableLanguageIdentification: true };
    input.deepgramOptionsOverride = { diarize: true, multichannel: false };
    expect(stt.getState().addFiles([input, request("c:\\AUDIO\\ONE.WAV")])).toBe(1);
    input.sonioxOptionsOverride.languageHints!.push("ru");
    input.deepgramOptionsOverride.diarize = false;
    const item = stt.getState().items[0];
    expect(item.request.saveToFile).toBe(true);
    expect(item.request.sonioxOptionsOverride?.languageHints).toEqual(["en"]);
    expect(item.request.deepgramOptionsOverride?.diarize).toBe(true);
    expect(item.name).toBe("one.wav");
    expect(calls).toHaveLength(0);
  });

  test("on Linux, paths differing only by letter case are queued separately", () => {
    (window as any).__TAURI_OS_PLUGIN_INTERNALS__.os_type = "linux";
    expect(stt.getState().addFiles([request("/audio/One.wav"), request("/audio/one.wav"), request("/audio/one.wav")])).toBe(2);
    expect(stt.getState().items.map((item) => item.path)).toEqual(["/audio/One.wav", "/audio/one.wav"]);
  });

  test("files appended during a run execute sequentially with their saved request options", async () => {
    const job = deferred<any>();
    const transcriptionCalls: any[] = [];
    backend = async (command, args) => {
      if (command === "get_file_transcription_recording_state") return { blocksFileTranscription: false };
      if (command === "transcribe_audio_file") {
        transcriptionCalls.push(args);
        return transcriptionCalls.length === 1 ? job.promise : { text: "two", saved_file_path: "Documents/two.srt" };
      }
    };
    stt.getState().addFiles([request("one.wav")]);
    const run = stt.getState().start();
    await until(() => transcriptionCalls.length === 1);
    stt.getState().addFiles([{ ...request("two.wav"), outputFormat: "srt", customWordsEnabledOverride: false }]);
    await stt.getState().start();
    expect(transcriptionCalls).toHaveLength(1);
    job.resolve({ text: "one", saved_file_path: "Documents/one.txt" });
    await run;
    expect(transcriptionCalls.map((args) => [args.filePath, args.saveToFile, args.outputFormat, args.customWordsEnabledOverride])).toEqual([
      ["one.wav", true, "text", true], ["two.wav", true, "srt", false],
    ]);
    expect(stt.getState().items.map((item) => item.savedFilePath)).toEqual(["Documents/one.txt", "Documents/two.srt"]);
    expect(stt.getState().items.every((item) => item.status === "completed")).toBe(true);
  });

  test("a failed file does not block later files, and retry only requeues failures", async () => {
    let fail = true;
    backend = async (command, args) => {
      if (command === "get_file_transcription_recording_state") return { blocksFileTranscription: false };
      if (command === "transcribe_audio_file") {
        if (args.filePath === "bad.wav" && fail) throw "Quota exceeded";
        return { text: "ok", saved_file_path: "Documents/ok.txt" };
      }
    };
    stt.getState().addFiles([request("bad.wav"), request("good.wav")]);
    await stt.getState().start();
    expect(stt.getState().items.map((item) => item.status)).toEqual(["failed", "completed"]);
    expect(stt.getState().items[0].error).toBe("Quota exceeded");
    stt.getState().retryFailed();
    expect(stt.getState().items.map((item) => item.status)).toEqual(["queued", "completed"]);
    fail = false;
    await stt.getState().start();
    expect(calls.filter((call) => call.command === "transcribe_audio_file").map((call) => call.args.filePath)).toEqual(["bad.wav", "good.wav", "bad.wav"]);
  });

  test("Stop requeues the interrupted file and waits for a late cancellation before unlocking", async () => {
    const job = deferred<any>(), cancel = deferred<void>();
    backend = async (command) => {
      if (command === "get_file_transcription_recording_state") return { blocksFileTranscription: false };
      if (command === "transcribe_audio_file") return job.promise;
      if (command === "cancel_file_transcription") return cancel.promise;
    };
    stt.getState().addFiles([request("one.wav"), request("two.wav")]);
    const run = stt.getState().start();
    await until(() => !!stt.getState().activeItemId);
    const stop = stt.getState().stop();
    job.reject("File transcription was cancelled");
    await until(() => stt.getState().activeItemId === null);
    expect(stt.getState().running).toBe(true);
    expect(stt.getState().items.map((item) => item.status)).toEqual(["queued", "queued"]);
    await stt.getState().start();
    expect(calls.filter((call) => call.command === "transcribe_audio_file")).toHaveLength(1);
    cancel.resolve();
    await Promise.all([run, stop]);
    expect(stt.getState().running).toBe(false);
    expect(stt.getState().stopping).toBe(false);
  });

  test("a successful result racing Stop stays completed and cancellation errors remain visible", async () => {
    const job = deferred<any>();
    backend = async (command) => {
      if (command === "get_file_transcription_recording_state") return { blocksFileTranscription: false };
      if (command === "transcribe_audio_file") return job.promise;
      if (command === "cancel_file_transcription") throw new Error("Cancel unavailable");
    };
    stt.getState().addFiles([request("one.wav"), request("two.wav")]);
    const run = stt.getState().start();
    await until(() => !!stt.getState().activeItemId);
    await stt.getState().stop();
    job.resolve({ text: "done", saved_file_path: "Documents/one.txt" });
    await run;
    expect(stt.getState().items.map((item) => item.status)).toEqual(["completed", "queued"]);
    expect(stt.getState().error).toContain("Cancel unavailable");
  });

  test("an admission check failure pauses without consuming the queued file", async () => {
    backend = async () => { throw new Error("Recording state unavailable"); };
    stt.getState().addFiles([request("one.wav")]);
    await stt.getState().start();
    expect(stt.getState().items[0].status).toBe("queued");
    expect(stt.getState().running).toBe(false);
    expect(stt.getState().error).toContain("Recording state unavailable");
    expect(calls.some((call) => call.command === "transcribe_audio_file")).toBe(false);
  });

  test("a pending single-file cancellation blocks the queue without being cancelled by Stop", async () => {
    single.setState({ isCancellationCommandPending: true });
    stt.getState().addFiles([request("one.wav")]);
    const run = stt.getState().start();
    expect(stt.getState().waitingReason).toBe("singleFile");
    await stt.getState().stop();
    await run;
    expect(calls).toHaveLength(0);
    expect(stt.getState().items[0].status).toBe("queued");
  });

  test("local recording blocks admission until it finishes", async () => {
    let blocked = true;
    backend = async (command) => {
      if (command === "get_file_transcription_recording_state") return { blocksFileTranscription: blocked };
      if (command === "transcribe_audio_file") return { text: "done", saved_file_path: "Documents/one.txt" };
    };
    stt.getState().addFiles([request("one.wav")]);
    const run = stt.getState().start();
    await until(() => stt.getState().waitingReason === "recording");
    expect(calls.some((call) => call.command === "transcribe_audio_file")).toBe(false);
    blocked = false;
    await run;
    expect(stt.getState().items[0].status).toBe("completed");
  });

  test("removing a file during its admission check never sends it to the provider", async () => {
    const admission = deferred<any>();
    backend = async () => admission.promise;
    stt.getState().addFiles([request("one.wav")]);
    const run = stt.getState().start();
    stt.getState().removeItem(stt.getState().items[0].id);
    admission.resolve({ blocksFileTranscription: false });
    await run;
    expect(stt.getState().items).toHaveLength(0);
    expect(calls.some((call) => call.command === "transcribe_audio_file")).toBe(false);
  });

  test("active items cannot be removed, and clear finished preserves pending work", async () => {
    const job = deferred<any>();
    backend = async (command) => command === "get_file_transcription_recording_state" ? { blocksFileTranscription: false } : job.promise;
    stt.getState().addFiles([request("one.wav"), request("two.wav")]);
    const run = stt.getState().start();
    await until(() => !!stt.getState().activeItemId);
    stt.getState().removeItem(stt.getState().activeItemId!);
    expect(stt.getState().items).toHaveLength(2);
    const stop = stt.getState().stop();
    job.resolve({ text: "done", saved_file_path: "Documents/one.txt" });
    await Promise.all([run, stop]);
    stt.getState().clearFinished();
    expect(stt.getState().items.map((item) => item.path)).toEqual(["two.wav"]);
  });

  test("rehydration turns processing into queued work without auto-starting", async () => {
    stt.getState().addFiles([request("one.wav"), request("two.wav")]);
    stt.setState((state) => ({
      running: true, stopping: true,
      items: state.items.map((item, index) => ({ ...item, status: index === 0 ? "completed" : "processing" })),
    }));
    const persisted = storage.get("aivorelay-transcribe-file-batch-v1")!;
    expect(Object.keys(JSON.parse(persisted).state)).toEqual(["items"]);
    stt.setState(initialStt, true);
    storage.set("aivorelay-transcribe-file-batch-v1", persisted);
    await stt.persist.rehydrate();
    expect(stt.getState().items.map((item) => item.status)).toEqual(["completed", "queued"]);
    expect(stt.getState().running).toBe(false);
    expect(stt.getState().stopping).toBe(false);
    expect(calls).toHaveLength(0);
  });

  test("retry does not duplicate a failed path that has already been added again", async () => {
    backend = async (command) => {
      if (command === "get_file_transcription_recording_state") return { blocksFileTranscription: false };
      throw "Bad audio";
    };
    stt.getState().addFiles([request("one.wav")]);
    await stt.getState().start();
    stt.getState().addFiles([request("ONE.WAV")]);
    stt.getState().retryFailed();
    expect(stt.getState().items.map((item) => item.status)).toEqual(["failed", "queued"]);
  });
});
