export const supportedExtensions = ["wav", "mp3", "m4a", "ogg", "flac", "webm"];

export function isSupportedAudioPath(path: string): boolean {
  const name = path.split(/[\\/]/).pop() ?? "";
  const dot = name.lastIndexOf(".");
  return dot > 0 && supportedExtensions.includes(name.slice(dot + 1).toLowerCase());
}

export function shouldRouteAudioDrop(section: string, paths: string[]): boolean {
  return section !== "transcribeFile" && section !== "textToSpeech" && section !== "ttsFiles" && paths.some(isSupportedAudioPath);
}

export function shouldQueueIncomingAudio(pathCount: number, hasSelectedFile: boolean, phase: string, batchRunning: boolean): boolean {
  return pathCount > 1 || hasSelectedFile || phase === "transcribing" || phase === "cancelling" || batchRunning;
}

let pendingPaths: string[] = [];

export function queueIncomingAudio(paths: string[]): void {
  pendingPaths.push(...paths);
}

export function takeIncomingAudio(): string[] {
  const paths = pendingPaths;
  pendingPaths = [];
  return paths;
}
