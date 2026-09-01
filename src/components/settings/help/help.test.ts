import { expect, it } from "bun:test";
import type { SidebarSection } from "../../Sidebar";
import { HELP_SECTIONS } from "./helpContent";
import { searchHelp } from "./helpSearch";

it("normalizes search queries across casing, diacritics, and surrounding whitespace while rejecting empty input", () => {
  // Empty and whitespace-only queries return an empty array
  expect(searchHelp("")).toEqual([]);
  expect(searchHelp("   ")).toEqual([]);
  expect(searchHelp(" \t\n ")).toEqual([]);

  // Unmatched queries return no results
  expect(searchHelp("nonexistent_unmatched_query_xyz")).toEqual([]);

  // Case-insensitivity: uppercase and lowercase return the expected target anchor
  const upperResults = searchHelp("TTS");
  const lowerResults = searchHelp("tts");
  expect(upperResults.length).toBeGreaterThan(0);
  expect(upperResults[0].anchor).toBe("help-speak-selected-text");
  expect(upperResults[0].score).toBe(lowerResults[0]?.score);

  // Diacritic / accent normalization: accented queries match unaccented records
  const diacriticResults = searchHelp("módel");
  expect(diacriticResults.some(r => r.anchor === "help-models")).toBe(true);

  // Multi-token query with irregular whitespace splits tokens cleanly
  const multiTokenResults = searchHelp("  live   monitor  ");
  expect(multiTokenResults[0]?.anchor).toBe("help-live-monitor");
});

it("ranks exact destination and title matches above indirect keyword matches in descending score order", () => {
  // Exact destination query boosts score above secondary keyword matches
  const historyResults = searchHelp("history");
  expect(historyResults.length).toBeGreaterThan(1);
  expect(historyResults[0].anchor).toBe("help-history");
  expect(historyResults[0].score).toBeGreaterThan(historyResults[1].score);

  // Exact destination for models outranks mentions in other sections
  const modelResults = searchHelp("models");
  expect(modelResults[0].anchor).toBe("help-models");

  // Every result set is strictly ordered in descending score sequence
  const audioResults = searchHelp("audio");
  expect(audioResults.length).toBeGreaterThan(1);
  for (let i = 0; i < audioResults.length - 1; i++) {
    expect(audioResults[i].score).toBeGreaterThanOrEqual(audioResults[i + 1].score);
  }

  // All returned results have strictly positive scores
  expect(audioResults.every(r => r.score > 0)).toBe(true);
});

it("maintains catalog integrity with unique anchors and valid sidebar navigation destinations", () => {
  expect(HELP_SECTIONS.length).toBeGreaterThan(0);

  const allAnchors: string[] = [];
  const validSidebarDestinations = new Set<SidebarSection>([
    "general",
    "models",
    "advanced",
    "postprocessing",
    "aiReplace",
    "sendSelectedText",
    "voiceCommands",
    "browserConnector",
    "textReplacement",
    "userInterface",
    "history",
    "audioProcessing",
    "debug",
    "liveSoundTranscription",
    "transcribeFile",
    "textToSpeech",
    "ttsFiles",
    "help",
    "about",
  ]);

  for (const section of HELP_SECTIONS) {
    // Top-level section properties
    expect(section.id.length).toBeGreaterThan(0);
    expect(section.anchor.startsWith("help-")).toBe(true);
    expect(section.titleKey.startsWith("help.sections.")).toBe(true);
    expect(section.summaryKey.startsWith("help.sections.")).toBe(true);
    expect(section.destinationLabelKey.startsWith("sidebar.")).toBe(true);
    expect(validSidebarDestinations.has(section.destination)).toBe(true);

    allAnchors.push(section.anchor);

    for (const subsection of section.subsections ?? []) {
      expect(subsection.id.length).toBeGreaterThan(0);
      expect(subsection.anchor.startsWith("help-")).toBe(true);
      expect(subsection.titleKey.startsWith("help.sections.")).toBe(true);
      expect(subsection.summaryKey.startsWith("help.sections.")).toBe(true);
      expect(subsection.destinationLabelKey.startsWith("sidebar.")).toBe(true);
      expect(validSidebarDestinations.has(subsection.destination)).toBe(true);

      allAnchors.push(subsection.anchor);
    }
  }

  // All anchors across sections and subsections must be unique
  const uniqueAnchors = new Set(allAnchors);
  expect(uniqueAnchors.size).toBe(allAnchors.length);

  // Search results for common help queries only resolve to valid catalog anchors
  const searchResults = searchHelp("transcription");
  expect(searchResults.length).toBeGreaterThan(0);
  expect(searchResults.every(r => uniqueAnchors.has(r.anchor))).toBe(true);
});

it("preserves subsection structural hierarchy, dedicated sub-anchors, and contextual warnings", () => {
  const transcriptionSection = HELP_SECTIONS.find(s => s.id === "transcription");
  expect(transcriptionSection).toBeDefined();
  expect(transcriptionSection?.subsections).toBeDefined();

  const subsections = transcriptionSection!.subsections!;
  expect(subsections.length).toBe(3);

  // Subsections map to distinct, specialized sidebar targets
  const subsectionIds = subsections.map(sub => sub.id);
  expect(subsectionIds).toEqual(["models", "speechProcessing", "advanced"]);

  const destinations = subsections.map(sub => sub.destination);
  expect(destinations).toEqual(["models", "audioProcessing", "advanced"]);

  // Subsection anchors are strictly distinct from parent section anchor
  for (const sub of subsections) {
    expect(sub.anchor).not.toBe(transcriptionSection!.anchor);
    expect(sub.anchor.startsWith("help-")).toBe(true);
  }

  // Model subsection contains security/cost warning key while others do not
  const modelsSubsection = subsections.find(sub => sub.id === "models");
  expect(modelsSubsection?.warningKey).toBe("help.sections.transcription.models.warning");

  const speechSubsection = subsections.find(sub => sub.id === "speechProcessing");
  expect(speechSubsection?.warningKey).toBeUndefined();

  // Non-hierarchical sections omit subsections
  const historySection = HELP_SECTIONS.find(s => s.id === "history");
  expect(historySection?.subsections).toBeUndefined();
});
