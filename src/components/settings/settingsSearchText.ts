const normalizeSearchFragment = (value: string): string =>
  value
    .toLocaleLowerCase()
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[\s_\\/\-\u2013\u2014]+/g, " ");

export const normalizeSearchText = (value: string): string =>
  normalizeSearchFragment(value).trim().replace(/\s+/g, " ");

export const findNormalizedMatchRange = (
  text: string,
  query: string,
): [number, number] | null => {
  const normalizedQuery = normalizeSearchText(query);
  if (!normalizedQuery) return null;

  const sourceRanges: Array<[number, number]> = [];
  let normalizedText = "";
  let sourceOffset = 0;

  for (const character of text) {
    const characterEnd = sourceOffset + character.length;
    const normalizedCharacter = normalizeSearchFragment(character);

    for (const normalizedPart of normalizedCharacter) {
      if (normalizedPart === " ") {
        if (normalizedText.length === 0 || normalizedText.endsWith(" ")) {
          continue;
        }
      }

      normalizedText += normalizedPart;
      sourceRanges.push([sourceOffset, characterEnd]);
    }
    sourceOffset = characterEnd;
  }

  if (normalizedText.endsWith(" ")) {
    normalizedText = normalizedText.slice(0, -1);
    sourceRanges.pop();
  }

  const normalizedStart = normalizedText.indexOf(normalizedQuery);
  if (normalizedStart < 0) return null;

  const firstRange = sourceRanges[normalizedStart];
  const lastRange = sourceRanges[normalizedStart + normalizedQuery.length - 1];
  return firstRange && lastRange ? [firstRange[0], lastRange[1]] : null;
};
