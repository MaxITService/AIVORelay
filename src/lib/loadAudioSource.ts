/** A recording read may finish after its History row has been removed. */
export async function loadAudioSource(
  load: () => Promise<string | null>,
  isCurrent: () => boolean,
): Promise<string | null> {
  const source = await load();
  if (!isCurrent()) {
    if (source?.startsWith("blob:")) URL.revokeObjectURL(source);
    return null;
  }
  return source;
}
