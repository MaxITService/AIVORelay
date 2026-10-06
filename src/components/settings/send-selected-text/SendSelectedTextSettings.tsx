import React, {
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  Check,
  ChevronDown,
  ChevronUp,
  ClipboardCopy,
  Clock3,
  Code2,
  Copy,
  FileJson,
  FileText,
  FolderOpen,
  History,
  Lightbulb,
  Play,
  Plus,
  Save,
  Send,
  Settings2,
  Trash2,
} from "lucide-react";
import {
  commands,
  type ExecutionPolicy,
  type Result,
  type SendSelectedTextCaptureMode as CaptureMode,
  type SendSelectedTextFormat as OutputFormat,
  type SendSelectedTextHistoryEntry,
  type SendSelectedTextHistoryStatus as HistoryStatus,
  type SendSelectedTextOversizeBehavior as OversizeBehavior,
  type SendSelectedTextPreset as PersistedSendSelectedTextPreset,
  type SendSelectedTextSettings as PersistedSendSelectedTextFeatureSettings,
  type SendSelectedTextWriteMode as WriteMode,
} from "@/bindings";
import { useSettings } from "@/hooks/useSettings";
import { useSortedDisplayNames } from "@/hooks/useSortedDisplayNames";
import { useListSortDirection } from "@/hooks/useListSortPreference";
import { sessionToast as toast } from "@/lib/sessionToast";
import { shouldUseSystem12HourClock } from "@/utils/dateFormat";
import { Button } from "../../ui/Button";
import { Collapse } from "../../ui/Collapse";
import { Input } from "../../ui/Input";
import { NameSortControl } from "../../ui/NameSortControl";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { HandyShortcut } from "../HandyShortcut";
import "./SendSelectedTextSettings.css";

type SendSelectedTextPreset = Required<PersistedSendSelectedTextPreset>;
type SendSelectedTextFeatureSettings = Omit<
  Required<PersistedSendSelectedTextFeatureSettings>,
  "presets"
> & { presets: SendSelectedTextPreset[] };

const unwrapCommandResult = <T,>(result: Result<T, string>): T => {
  if (result.status === "error") throw result.error;
  return result.data;
};

// Rust fills serde-defaulted fields before returning settings; the generated
// input-compatible types remain optional so older stores can still load.
const asCompletePreset = (
  preset: PersistedSendSelectedTextPreset,
): SendSelectedTextPreset => preset as SendSelectedTextPreset;

const asCompleteFeatureSettings = (
  settings: PersistedSendSelectedTextFeatureSettings,
): SendSelectedTextFeatureSettings =>
  settings as SendSelectedTextFeatureSettings;

type PageTab = "presets" | "history" | "help";
const HISTORY_PAGE_SIZE = 100;

const STATUS_LABEL_KEYS: Record<HistoryStatus, string> = {
  saved: "sendSelectedText.status.saved",
  command_started: "sendSelectedText.status.commandStarted",
  completed: "sendSelectedText.status.completed",
  command_failed: "sendSelectedText.status.commandFailed",
  failed: "sendSelectedText.status.failed",
};

const WRITE_MODE_LABEL_KEYS: Record<WriteMode, string> = {
  create_new: "sendSelectedText.writeModes.createNew",
  append_last: "sendSelectedText.writeModes.appendLast",
  append_file: "sendSelectedText.writeModes.appendFile",
  overwrite_file: "sendSelectedText.writeModes.overwriteFile",
};

const getCopyExamples = (t: TFunction) => [
  {
    title: t("sendSelectedText.help.fileExamples.selection.title"),
    summary: t("sendSelectedText.help.fileExamples.selection.summary"),
    fields: [
      [t("sendSelectedText.fields.format"), "Markdown"],
      [t("sendSelectedText.fields.fileAction"), t(WRITE_MODE_LABEL_KEYS.create_new)],
      [t("sendSelectedText.fields.filename"), "selected-{{date}}-{{time}}.md"],
      [t("sendSelectedText.fields.content"), "{{text}}"],
    ],
  },
  {
    title: t("sendSelectedText.help.fileExamples.daily.title"),
    summary: t("sendSelectedText.help.fileExamples.daily.summary"),
    fields: [
      [t("sendSelectedText.fields.format"), "Markdown"],
      [t("sendSelectedText.fields.fileAction"), t(WRITE_MODE_LABEL_KEYS.append_file)],
      [t("sendSelectedText.fields.filename"), "inbox-{{date}}.md"],
      [t("sendSelectedText.fields.content"), "## {{timestamp_local}}\n\n{{text}}"],
    ],
  },
  {
    title: t("sendSelectedText.help.fileExamples.lastFile.title"),
    summary: t("sendSelectedText.help.fileExamples.lastFile.summary"),
    fields: [
      [t("sendSelectedText.fields.format"), "Markdown"],
      [t("sendSelectedText.fields.fileAction"), t(WRITE_MODE_LABEL_KEYS.append_last)],
      [t("sendSelectedText.fields.filename"), "task-{{date}}-{{time}}.md"],
      [t("sendSelectedText.fields.content"), "{{text}}"],
    ],
  },
  {
    title: t("sendSelectedText.help.fileExamples.json.title"),
    summary: t("sendSelectedText.help.fileExamples.json.summary"),
    fields: [
      [t("sendSelectedText.fields.format"), "JSON"],
      [t("sendSelectedText.fields.fileAction"), t(WRITE_MODE_LABEL_KEYS.append_file)],
      [t("sendSelectedText.fields.filename"), "selected-text.json"],
      [t("sendSelectedText.fields.keepLatest"), "50"],
    ],
  },
];

const getCommandExamples = (t: TFunction) => [
  {
    title: t("sendSelectedText.help.commandExamples.explain.title"),
    description: t("sendSelectedText.help.commandExamples.explain.description"),
    command:
      "Get-Content -Raw -Encoding UTF8 -LiteralPath {{input_file}} | codex exec -C {{working_directory}} -s read-only -",
  },
  {
    title: t("sendSelectedText.help.commandExamples.implement.title"),
    description: t("sendSelectedText.help.commandExamples.implement.description"),
    command:
      "Get-Content -Raw -Encoding UTF8 -LiteralPath {{input_file}} | codex exec -C {{working_directory}} -s workspace-write -",
  },
  {
    title: t("sendSelectedText.help.commandExamples.process.title"),
    description: t("sendSelectedText.help.commandExamples.process.description"),
    command:
      '("Process the supplied text. Return only the useful final result.`n`n" + (Get-Content -Raw -Encoding UTF8 -LiteralPath {{input_file}})) | codex exec -s read-only -',
  },
  {
    title: t("sendSelectedText.help.commandExamples.cleanup.title"),
    description: t("sendSelectedText.help.commandExamples.cleanup.description"),
    command:
      '("Remove repetition, boilerplate, and irrelevant material. Return only the cleaned text.`n`n" + (Get-Content -Raw -Encoding UTF8 -LiteralPath {{input_file}})) | codex exec -s read-only -',
  },
  {
    title: t("sendSelectedText.help.commandExamples.summarize.title"),
    description: t("sendSelectedText.help.commandExamples.summarize.description"),
    command:
      '("Summarize the supplied text as concise Markdown bullets.`n`n" + (Get-Content -Raw -Encoding UTF8 -LiteralPath {{input_file}})) | codex exec -s read-only -',
  },
  {
    title: t("sendSelectedText.help.commandExamples.inbox.title"),
    description: t("sendSelectedText.help.commandExamples.inbox.description"),
    command:
      'codex exec -C {{working_directory}} -s workspace-write ("Review the inbox file at " + {{file_path}} + ". Organize it, preserve useful content, and remove duplicates.")',
  },
  {
    title: t("sendSelectedText.help.commandExamples.agent.title"),
    description: t("sendSelectedText.help.commandExamples.agent.description"),
    command: "my-agent --input-file {{input_file}} --output-file {{file_path}}",
  },
  {
    title: t("sendSelectedText.help.commandExamples.directText.title"),
    description: t("sendSelectedText.help.commandExamples.directText.description", {
      textVariable: "{{text}}",
    }),
    command: 'codex exec -s read-only ("Solve this task: " + {{text}})',
  },
];

const getVariables = (t: TFunction) => [
  ["{{input_file}}", t("sendSelectedText.help.variables.inputFile")],
  ["{{file_path}}", t("sendSelectedText.help.variables.filePath")],
  ["{{directory}}", t("sendSelectedText.help.variables.directory")],
  ["{{filename}}", t("sendSelectedText.help.variables.filename")],
  ["{{text}}", t("sendSelectedText.help.variables.text")],
  ["{{record_id}}", t("sendSelectedText.help.variables.recordId")],
  ["{{timestamp}}", t("sendSelectedText.help.variables.timestamp")],
  ["{{timestamp_local}}", t("sendSelectedText.help.variables.timestampLocal")],
  ["{{date}} / {{time}}", t("sendSelectedText.help.variables.dateTime")],
  ["{{preset_id}} / {{preset_name}}", t("sendSelectedText.help.variables.preset")],
  ["{{format}} / {{write_mode}}", t("sendSelectedText.help.variables.strategy")],
  ["{{text_length}}", t("sendSelectedText.help.variables.textLength")],
  ["{{working_directory}}", t("sendSelectedText.help.variables.workingDirectory")],
];

const formatDate = (timestampMs: number, language: string) =>
  new Intl.DateTimeFormat(language, {
    dateStyle: "medium",
    timeStyle: "medium",
    hour12: shouldUseSystem12HourClock(),
  }).format(new Date(timestampMs));

const previewText = (value: string, maximum = 220) => {
  const compact = value.replace(/\s+/g, " ").trim();
  return compact.length > maximum ? `${compact.slice(0, maximum)}...` : compact;
};

function FieldLabel({ children }: { children: React.ReactNode }) {
  return <span className="sst-field-label">{children}</span>;
}

function CopyButton({ value, label }: { value: string; label: string }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1400);
    } catch (error) {
      toast.error(t("sendSelectedText.messages.copyFailed", { label, error: String(error) }));
    }
  };
  return (
    <Button
      type="button"
      variant="secondary"
      size="sm"
      className="flex min-h-8 min-w-8 items-center justify-center !p-1.5"
      onClick={handleCopy}
      title={t("sendSelectedText.actions.copy", { label })}
      aria-label={t("sendSelectedText.actions.copy", { label })}
    >
      {copied ? <Check size={15} /> : <Copy size={15} />}
    </Button>
  );
}

interface PresetCardProps {
  preset: SendSelectedTextPreset;
  onSave: (preset: SendSelectedTextPreset) => Promise<SendSelectedTextPreset>;
  onDelete: (preset: SendSelectedTextPreset) => Promise<void>;
  onDuplicate: (preset: SendSelectedTextPreset) => Promise<void>;
  onRunSample: (preset: SendSelectedTextPreset, text: string) => Promise<void>;
  onTrimJson: (preset: SendSelectedTextPreset) => Promise<void>;
}

function PresetCard({
  preset,
  onSave,
  onDelete,
  onDuplicate,
  onRunSample,
  onTrimJson,
}: PresetCardProps) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState(preset);
  const [dirty, setDirty] = useState(false);
  const draftRevision = useRef(0);
  const busyRef = useRef(false);
  const [expanded, setExpanded] = useState(true);
  const [busy, setBusy] = useState(false);
  const [customSampleText, setSampleText] = useState<string | null>(null);
  const sampleText = customSampleText ?? t("sendSelectedText.presets.sampleText");

  useEffect(() => {
    if (!dirty) setDraft(preset);
  }, [dirty, preset]);

  const update = <K extends keyof SendSelectedTextPreset>(
    key: K,
    value: SendSelectedTextPreset[K],
  ) => {
    draftRevision.current += 1;
    setDirty(true);
    setDraft((current) => ({ ...current, [key]: value }));
  };

  const saveDraft = async () => {
    if (busyRef.current) return;
    busyRef.current = true;
    const revision = draftRevision.current;
    setBusy(true);
    try {
      const saved = await onSave(draft);
      if (draftRevision.current === revision) {
        setDraft(saved);
        setDirty(false);
        toast.success(t("sendSelectedText.messages.presetSaved"));
      } else {
        toast.success(t("sendSelectedText.messages.presetSavedWithNewerEdits"));
      }
    } catch {
      // The parent reports the save error.
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  };

  const runSampleDraft = async () => {
    if (busyRef.current) return;
    busyRef.current = true;
    const revision = draftRevision.current;
    setBusy(true);
    try {
      await onRunSample(draft, sampleText);
      if (draftRevision.current === revision) setDirty(false);
    } catch {
      // The parent reports the run error.
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  };

  const trimJsonDraft = async () => {
    if (busyRef.current) return;
    busyRef.current = true;
    const revision = draftRevision.current;
    setBusy(true);
    try {
      await onTrimJson(draft);
      if (draftRevision.current === revision) setDirty(false);
    } catch {
      // The parent reports the trim error.
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  };

  const duplicateDraft = async () => {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    try {
      await onDuplicate(draft);
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  };

  const deleteDraft = async () => {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    try {
      await onDelete(preset);
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  };

  const chooseDirectory = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: draft.destination_directory || undefined,
    });
    if (typeof selected === "string") update("destination_directory", selected);
  };

  const setFormat = (format: OutputFormat) => {
    draftRevision.current += 1;
    setDirty(true);
    setDraft((current) => {
      const wasDefaultMarkdown =
        current.filename_template === "selected-{{date}}-{{time}}.md";
      const wasDefaultJson = current.filename_template === "selected-text.json";
      return {
        ...current,
        format,
        write_mode:
          format === "json" && current.write_mode === "append_last"
            ? "append_file"
            : current.write_mode,
        filename_template:
          format === "json" && wasDefaultMarkdown
            ? "selected-text.json"
            : format === "markdown" && wasDefaultJson
              ? "selected-{{date}}-{{time}}.md"
              : current.filename_template,
      };
    });
  };

  const writeModes: WriteMode[] =
    draft.format === "json"
      ? ["create_new", "append_file", "overwrite_file"]
      : ["create_new", "append_last", "append_file", "overwrite_file"];

  return (
    <article className="sst-preset-card">
      <header className="sst-preset-header">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            {draft.format === "json" ? (
              <FileJson size={17} className="text-[#65d6a6]" />
            ) : (
              <FileText size={17} className="text-[#84b8ff]" />
            )}
            <h3 className="truncate text-sm font-semibold text-[#f2f2f2]">
              {preset.name}
            </h3>
            {!preset.enabled && (
              <span className="sst-status-chip neutral">{t("sendSelectedText.presets.disabled")}</span>
            )}
          </div>
          <p className="mt-1 truncate text-xs text-[#969696]">
            {t(WRITE_MODE_LABEL_KEYS[preset.write_mode])}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <Button
            type="button"
            variant="secondary"
            size="sm"
            className="flex min-h-8 min-w-8 items-center justify-center !p-1.5"
            disabled={busy}
            onClick={duplicateDraft}
            title={t("sendSelectedText.actions.duplicatePreset")}
            aria-label={t("sendSelectedText.actions.duplicatePreset")}
          >
            <ClipboardCopy size={15} />
          </Button>
          <Button
            type="button"
            variant="danger"
            size="sm"
            className="flex min-h-8 min-w-8 items-center justify-center !p-1.5"
            disabled={busy}
            onClick={deleteDraft}
            title={t("sendSelectedText.actions.deletePreset")}
            aria-label={t("sendSelectedText.actions.deletePreset")}
          >
            <Trash2 size={15} />
          </Button>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            className="flex min-h-8 min-w-8 items-center justify-center !p-1.5"
            onClick={() => setExpanded((value) => !value)}
            title={t(expanded ? "sendSelectedText.actions.collapsePreset" : "sendSelectedText.actions.expandPreset")}
            aria-label={t(expanded ? "sendSelectedText.actions.collapsePreset" : "sendSelectedText.actions.expandPreset")}
            aria-expanded={expanded}
          >
            {expanded ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
          </Button>
        </div>
      </header>

      <Collapse open={expanded}>
        <div className="sst-preset-body">
          <div className="sst-form-grid two">
            <label className="sst-field">
              <FieldLabel>{t("sendSelectedText.fields.presetName")}</FieldLabel>
              <Input
                variant="compact"
                className="w-full text-xs"
                value={draft.name}
                onChange={(event) => update("name", event.target.value)}
              />
            </label>
            <div className="sst-field">
              <FieldLabel>{t("sendSelectedText.fields.enabled")}</FieldLabel>
              <span className="sst-toggle-row">
                <ToggleSwitch
                  checked={draft.enabled}
                  onChange={(checked) => update("enabled", checked)}
                  ariaLabel={t("sendSelectedText.presets.enableNamed", { name: draft.name })}
                />
                <span>{t("sendSelectedText.presets.enabledHelp")}</span>
              </span>
            </div>
          </div>

          <div className="sst-hotkey-row">
            <div>
              <FieldLabel>{t("sendSelectedText.fields.hotkey")}</FieldLabel>
              <p>
                {t("sendSelectedText.presets.hotkeyHelp")}
              </p>
            </div>
            <HandyShortcut
              shortcutId={`send_selected_text_${preset.id}`}
              title={t("sendSelectedText.presets.hotkeyTitle", { name: preset.name })}
              description={t("sendSelectedText.presets.hotkeyDescription")}
              grouped
              descriptionMode="tooltip"
              disabled={!draft.enabled}
            />
          </div>

          <div className="sst-form-grid three">
            <label className="sst-field">
              <FieldLabel>{t("sendSelectedText.fields.format")}</FieldLabel>
              <select
                value={draft.format}
                onChange={(event) =>
                  setFormat(event.target.value as OutputFormat)
                }
              >
                <option value="markdown">Markdown</option>
                <option value="json">JSON</option>
              </select>
            </label>
            <label className="sst-field span-two">
              <FieldLabel>{t("sendSelectedText.fields.fileAction")}</FieldLabel>
              <select
                value={draft.write_mode}
                onChange={(event) =>
                  update("write_mode", event.target.value as WriteMode)
                }
              >
                {writeModes.map((mode) => (
                  <option key={mode} value={mode}>
                    {t(WRITE_MODE_LABEL_KEYS[mode])}
                  </option>
                ))}
              </select>
            </label>
          </div>

          <div className="sst-field">
            <FieldLabel>{t("sendSelectedText.fields.destinationFolder")}</FieldLabel>
            <div className="sst-input-action-row">
              <Input
                variant="compact"
                className="w-full text-xs"
                value={draft.destination_directory}
                onChange={(event) =>
                  update("destination_directory", event.target.value)
                }
                placeholder={t("sendSelectedText.presets.destinationPlaceholder")}
              />
              <Button
                type="button"
                variant="secondary"
                size="sm"
                className="flex min-h-[34px] items-center justify-center gap-2 whitespace-nowrap"
                onClick={chooseDirectory}
                title={t("sendSelectedText.actions.chooseFolder")}
              >
                <FolderOpen size={16} />
                <span>{t("sendSelectedText.actions.browse")}</span>
              </Button>
            </div>
          </div>

          <label className="sst-field">
            <FieldLabel>{t("sendSelectedText.fields.filenameTemplate")}</FieldLabel>
            <Input
              variant="compact"
              className="w-full font-mono text-xs"
              value={draft.filename_template}
              onChange={(event) =>
                update("filename_template", event.target.value)
              }
            />
            <small>
              {t("sendSelectedText.presets.filenameHelp", {
                variables: "{{date}}, {{time}}, {{record_id}}, {{preset_name}}",
              })}
            </small>
          </label>

          {draft.format === "markdown" ? (
            <label className="sst-field">
              <FieldLabel>{t("sendSelectedText.fields.contentTemplate")}</FieldLabel>
              <textarea
                className="min-h-[104px] font-mono"
                value={draft.content_template}
                onChange={(event) =>
                  update("content_template", event.target.value)
                }
              />
              <small>
                {t("sendSelectedText.presets.contentHelp", {
                  textVariable: "{{text}}",
                  optionalVariables: "{{timestamp_local}}, {{preset_name}}, {{record_id}}",
                })}
              </small>
            </label>
          ) : (
            <div className="sst-json-note">
              <FileJson size={18} />
              <div>
                <strong>{t("sendSelectedText.presets.jsonTitle")}</strong>
                <p>
                  {t("sendSelectedText.presets.jsonHelp")}
                </p>
              </div>
              <label>
                <span>{t("sendSelectedText.fields.keepLatest")}</span>
                <Input
                  type="number"
                  variant="compact"
                  className="w-full text-xs"
                  min={0}
                  max={100000}
                  value={draft.json_keep_last}
                  onChange={(event) =>
                    update("json_keep_last", Number(event.target.value))
                  }
                />
              </label>
              {draft.write_mode !== "create_new" &&
                draft.json_keep_last > 0 && (
                  <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    disabled={busy}
                    onClick={trimJsonDraft}
                  >
                    {t("sendSelectedText.actions.trimJson")}
                  </Button>
                )}
            </div>
          )}

          <div className="sst-form-grid three">
            <label className="sst-field">
              <FieldLabel>{t("sendSelectedText.fields.captureMethod")}</FieldLabel>
              <select
                value={draft.capture_mode}
                onChange={(event) =>
                  update("capture_mode", event.target.value as CaptureMode)
                }
              >
                <option value="auto">{t("sendSelectedText.captureModes.auto")}</option>
                <option value="clipboard_copy">
                  {t("sendSelectedText.captureModes.clipboardCopy")}
                </option>
                <option value="accessibility">
                  {t("sendSelectedText.captureModes.accessibility")}
                </option>
              </select>
            </label>
            <label className="sst-field">
              <FieldLabel>{t("sendSelectedText.fields.maximumCharacters")}</FieldLabel>
              <Input
                type="number"
                variant="compact"
                className="w-full text-xs"
                min={1}
                max={2000000}
                value={draft.max_chars}
                onChange={(event) =>
                  update("max_chars", Number(event.target.value))
                }
              />
            </label>
            <label className="sst-field">
              <FieldLabel>{t("sendSelectedText.fields.oversizeBehavior")}</FieldLabel>
              <select
                value={draft.oversize_behavior}
                onChange={(event) =>
                  update(
                    "oversize_behavior",
                    event.target.value as OversizeBehavior,
                  )
                }
              >
                <option value="reject">{t("sendSelectedText.oversizeBehaviors.reject")}</option>
                <option value="truncate">{t("sendSelectedText.oversizeBehaviors.truncate")}</option>
              </select>
            </label>
          </div>

          <section className="sst-command-section">
            <div className="sst-section-heading-row">
              <div>
                <h4>{t("sendSelectedText.command.title")}</h4>
                <p>{t("sendSelectedText.command.savedFileHelp")}</p>
              </div>
              <span className="sst-toggle-row compact">
                <ToggleSwitch
                  checked={draft.command_enabled}
                  onChange={(checked) => update("command_enabled", checked)}
                  ariaLabel={t("sendSelectedText.command.title")}
                />
                <span>{t("sendSelectedText.fields.enabled")}</span>
              </span>
            </div>
            {draft.command_enabled && (
              <div className="space-y-3">
                <label className="sst-field">
                  <FieldLabel>{t("sendSelectedText.fields.powerShellCommand")}</FieldLabel>
                  <textarea
                    className="min-h-[112px] font-mono"
                    value={draft.command}
                    onChange={(event) => update("command", event.target.value)}
                    placeholder="Get-Content -Raw -Encoding UTF8 -LiteralPath {{input_file}} | codex exec -s read-only -"
                  />
                  <small>
                    {t("sendSelectedText.command.placeholderHelp")}
                  </small>
                </label>
                <div className="sst-form-grid two">
                  <label className="sst-field">
                    <FieldLabel>{t("sendSelectedText.fields.workingDirectory")}</FieldLabel>
                    <Input
                      variant="compact"
                      className="w-full text-xs"
                      value={draft.command_working_directory}
                      onChange={(event) =>
                        update("command_working_directory", event.target.value)
                      }
                      placeholder={t("sendSelectedText.command.workingDirectoryPlaceholder")}
                    />
                  </label>
                  <label className="sst-field">
                    <FieldLabel>{t("sendSelectedText.fields.executionPolicy")}</FieldLabel>
                    <select
                      value={draft.command_execution_policy}
                      onChange={(event) =>
                        update(
                          "command_execution_policy",
                          event.target.value as ExecutionPolicy,
                        )
                      }
                    >
                      <option value="default">{t("sendSelectedText.command.systemDefault")}</option>
                      <option value="bypass">Bypass</option>
                      <option value="remote_signed">RemoteSigned</option>
                      <option value="unrestricted">Unrestricted</option>
                    </select>
                  </label>
                </div>
                <div className="sst-check-grid">
                  <label>
                    <input
                      type="checkbox"
                      checked={draft.command_silent}
                      onChange={(event) =>
                        update("command_silent", event.target.checked)
                      }
                    />
                    {t("sendSelectedText.command.captureOutput")}
                  </label>
                  <label>
                    <input
                      type="checkbox"
                      checked={draft.command_no_profile}
                      onChange={(event) =>
                        update("command_no_profile", event.target.checked)
                      }
                    />
                    {t("sendSelectedText.command.skipProfile")}
                  </label>
                  <label>
                    <input
                      type="checkbox"
                      checked={draft.command_use_pwsh}
                      onChange={(event) =>
                        update("command_use_pwsh", event.target.checked)
                      }
                    />
                    {t("sendSelectedText.command.usePwsh")}
                  </label>
                  <label>
                    <input
                      type="checkbox"
                      checked={draft.allow_text_variable}
                      onChange={(event) =>
                        update("allow_text_variable", event.target.checked)
                      }
                    />
                    {t("sendSelectedText.command.allowDirectText", { textVariable: "{{text}}" })}
                  </label>
                </div>
                {draft.allow_text_variable && (
                  <div className="sst-warning-row">
                    <AlertTriangle size={17} />
                    <span>
                      {t("sendSelectedText.command.directTextWarning", {
                        inputFileVariable: "{{input_file}}",
                      })}
                    </span>
                  </div>
                )}
              </div>
            )}
          </section>

          <section className="sst-sample-section">
            <div>
              <FieldLabel>{t("sendSelectedText.presets.sampleTitle")}</FieldLabel>
              <p>
                {t("sendSelectedText.presets.sampleHelp")}
              </p>
            </div>
            <textarea
              value={sampleText}
              onChange={(event) => setSampleText(event.target.value)}
            />
            <Button
              type="button"
              variant="secondary"
              size="sm"
              disabled={busy || !sampleText.trim()}
              onClick={runSampleDraft}
            >
              <span className="flex items-center gap-2">
                <Play size={14} /> {t("sendSelectedText.actions.testPreset")}
              </span>
            </Button>
          </section>

          <footer className="sst-card-actions">
            <Button
              type="button"
              variant="primary"
              disabled={busy}
              onClick={saveDraft}
            >
              <span className="flex items-center gap-2">
                <Save size={15} /> {t("sendSelectedText.actions.savePreset")}
              </span>
            </Button>
          </footer>
        </div>
      </Collapse>
    </article>
  );
}

function HistoryView({
  entries,
  loading,
  loadingMore,
  clearing,
  hasMore,
  onRefresh,
  onLoadMore,
  onDelete,
  onClear,
}: {
  entries: SendSelectedTextHistoryEntry[];
  loading: boolean;
  loadingMore: boolean;
  clearing: boolean;
  hasMore: boolean;
  onRefresh: () => Promise<void>;
  onLoadMore: () => Promise<void>;
  onDelete: (id: number) => Promise<void>;
  onClear: () => Promise<void>;
}) {
  const { t, i18n } = useTranslation();
  return (
    <div className="space-y-3">
      <div className="sst-toolbar">
        <div>
          <h2 id="settings-selected-text-history" tabIndex={-1}>{t("sendSelectedText.history.title")}</h2>
          <p>{t("sendSelectedText.history.description")}</p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="secondary"
            size="sm"
            disabled={loading || loadingMore || clearing}
            onClick={onRefresh}
          >
            {t("sendSelectedText.actions.refresh")}
          </Button>
          <Button
            variant="danger"
            size="sm"
            disabled={
              loading || loadingMore || clearing || entries.length === 0
            }
            onClick={onClear}
          >
            {t(clearing ? "sendSelectedText.history.clearing" : "sendSelectedText.actions.clearHistory")}
          </Button>
        </div>
      </div>
      {loading ? (
        <div className="sst-empty">{t("sendSelectedText.history.loading")}</div>
      ) : entries.length === 0 ? (
        <div className="sst-empty">
          <History size={22} />
          <span>{t("sendSelectedText.history.empty")}</span>
        </div>
      ) : (
        <div className="space-y-2">
          {entries.map((entry) => (
            <article key={entry.id} className="sst-history-entry">
              <header>
                <div className="min-w-0">
                  <div className="flex flex-wrap items-center gap-2">
                    <strong>{entry.preset_name}</strong>
                    <span className={`sst-status-chip ${entry.status}`}>
                      {t(STATUS_LABEL_KEYS[entry.status])}
                    </span>
                    <span className="sst-history-format">
                      {entry.output_format === "json" ? "JSON" : "Markdown"}
                    </span>
                  </div>
                  <time>{formatDate(entry.timestamp_ms, i18n.resolvedLanguage ?? i18n.language)}</time>
                </div>
                <Button
                  type="button"
                  variant="danger"
                  size="sm"
                  className="flex min-h-8 min-w-8 items-center justify-center !p-1.5"
                  disabled={clearing}
                  onClick={() => onDelete(entry.id)}
                  title={t("sendSelectedText.actions.deleteHistoryEntry")}
                  aria-label={t("sendSelectedText.actions.deleteHistoryEntry")}
                >
                  <Trash2 size={15} />
                </Button>
              </header>
              <div className="sst-history-block">
                <div className="sst-history-block-title">
                  <span>{t("sendSelectedText.history.selectedText")}</span>
                  <CopyButton
                    value={entry.selected_text}
                    label={t("sendSelectedText.copyLabels.selectedText")}
                  />
                </div>
                <p>{previewText(entry.selected_text) || t("sendSelectedText.history.noText")}</p>
              </div>
              {entry.output_path && (
                <div className="sst-history-path">
                  <span>{entry.output_path}</span>
                  <CopyButton value={entry.output_path} label={t("sendSelectedText.copyLabels.filePath")} />
                </div>
              )}
              {entry.command && (
                <details className="sst-history-details">
                  <summary>{t("sendSelectedText.history.command")}</summary>
                  <div className="sst-history-detail-content">
                    <pre>{entry.command}</pre>
                    <CopyButton value={entry.command} label={t("sendSelectedText.copyLabels.command")} />
                  </div>
                </details>
              )}
              {entry.command_output && (
                <details className="sst-history-details">
                  <summary>{t("sendSelectedText.history.commandOutput")}</summary>
                  <div className="sst-history-detail-content">
                    <pre>{entry.command_output}</pre>
                    <CopyButton
                      value={entry.command_output}
                      label={t("sendSelectedText.copyLabels.commandOutput")}
                    />
                  </div>
                  {entry.command_output_truncated && (
                    <small>
                      {t("sendSelectedText.history.outputTruncated")}
                    </small>
                  )}
                </details>
              )}
              {entry.error && (
                <div className="sst-history-error">
                  <div className="sst-history-block-title">
                    <span>{t("sendSelectedText.history.fullError")}</span>
                    <CopyButton value={entry.error} label={t("sendSelectedText.copyLabels.fullError")} />
                  </div>
                  <pre>{entry.error}</pre>
                </div>
              )}
            </article>
          ))}
          {hasMore && (
            <div className="flex justify-center pt-2">
              <Button
                variant="secondary"
                size="sm"
                disabled={loading || loadingMore || clearing}
                onClick={onLoadMore}
              >
                {t(loadingMore ? "sendSelectedText.history.loadingMore" : "sendSelectedText.actions.loadOlder")}
              </Button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function HelpView() {
  const { t } = useTranslation();
  return (
    <div className="space-y-5">
      <section className="sst-help-intro">
        <Lightbulb size={21} />
        <div>
          <h2>{t("sendSelectedText.help.workflowTitle")}</h2>
          <ol>
            <li>{t("sendSelectedText.help.steps.select")}</li>
            <li>{t("sendSelectedText.help.steps.hotkey")}</li>
            <li>{t("sendSelectedText.help.steps.capture")}</li>
            <li>{t("sendSelectedText.help.steps.save")}</li>
            <li>{t("sendSelectedText.help.steps.command")}</li>
            <li>
              {t("sendSelectedText.help.steps.history")}
            </li>
          </ol>
        </div>
      </section>

      <section>
        <div className="sst-help-heading">
          <FileText size={18} />
          <div>
            <h2>{t("sendSelectedText.help.fileRecipesTitle")}</h2>
            <p>{t("sendSelectedText.help.fileRecipesDescription")}</p>
          </div>
        </div>
        <div className="sst-example-grid">
          {getCopyExamples(t).map((example) => (
            <article key={example.title} className="sst-example-card">
              <h3>{example.title}</h3>
              <p>{example.summary}</p>
              <dl>
                {example.fields.map(([name, value]) => (
                  <React.Fragment key={name}>
                    <dt>{name}</dt>
                    <dd>
                      <code>{value}</code>
                      <CopyButton value={value} label={name.toLowerCase()} />
                    </dd>
                  </React.Fragment>
                ))}
              </dl>
            </article>
          ))}
        </div>
      </section>

      <section>
        <div className="sst-help-heading">
          <Code2 size={18} />
          <div>
            <h2>{t("sendSelectedText.help.commandsTitle")}</h2>
            <p>
              {t("sendSelectedText.help.commandsDescription")}
            </p>
          </div>
        </div>
        <div className="space-y-2">
          {getCommandExamples(t).map((example) => (
            <article key={example.title} className="sst-command-example">
              <div>
                <h3>{example.title}</h3>
                <p>{example.description}</p>
              </div>
              <pre>{example.command}</pre>
              <CopyButton value={example.command} label={t("sendSelectedText.copyLabels.command")} />
            </article>
          ))}
        </div>
      </section>

      <section>
        <div className="sst-help-heading">
          <Settings2 size={18} />
          <div>
            <h2 id="settings-selected-text-variables" tabIndex={-1}>{t("sendSelectedText.help.variablesTitle")}</h2>
            <p>{t("sendSelectedText.help.variablesDescription")}</p>
          </div>
        </div>
        <div className="sst-variable-table">
          {getVariables(t).map(([name, description]) => (
            <React.Fragment key={name}>
              <code>{name}</code>
              <span>{description}</span>
              <CopyButton value={name} label={t("sendSelectedText.copyLabels.variable")} />
            </React.Fragment>
          ))}
        </div>
      </section>

      <section className="sst-safety-help">
        <AlertTriangle size={20} />
        <div>
          <h2>{t("sendSelectedText.help.privacyTitle")}</h2>
          <p>
            {t("sendSelectedText.help.executionNotes")}
          </p>
          <p>
            {t("sendSelectedText.help.historyNotes")}
          </p>
        </div>
      </section>
    </div>
  );
}

export default function SendSelectedTextSettings() {
  const { t } = useTranslation();
  const { refreshSettings } = useSettings();
  const [tab, setTab] = useState<PageTab>("presets");
  const [presetSortDirection, setPresetSortDirection] =
    useListSortDirection("send-selected-text-presets");
  const [feature, setFeature] =
    useState<SendSelectedTextFeatureSettings | null>(null);
  const [historyEntries, setHistoryEntries] = useState<
    SendSelectedTextHistoryEntry[]
  >([]);
  const [loading, setLoading] = useState(true);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyLoadingMore, setHistoryLoadingMore] = useState(false);
  const [historyClearing, setHistoryClearing] = useState(false);
  const [historyHasMore, setHistoryHasMore] = useState(false);
  const [optionsSaving, setOptionsSaving] = useState(false);
  const [creatingPreset, setCreatingPreset] = useState(false);
  const featureGeneration = useRef(0);
  const historyGeneration = useRef(0);
  const optionsDirty = useRef(false);
  const optionsRevision = useRef(0);
  const optionsSaveInFlight = useRef(false);
  const createPresetInFlight = useRef(false);
  const historyClearInFlight = useRef(false);
  const [optionsDraft, setOptionsDraft] = useState({
    historyLimit: 200,
    errorSeconds: 10,
  });

  const loadFeature = useCallback(async () => {
    const generation = featureGeneration.current + 1;
    featureGeneration.current = generation;
    const value = asCompleteFeatureSettings(
      await commands.getSendSelectedTextSettings(),
    );
    if (featureGeneration.current !== generation) return;
    setFeature(value);
    if (!optionsDirty.current) {
      setOptionsDraft({
        historyLimit: value.history_limit,
        errorSeconds: Math.round(value.error_overlay_auto_hide_ms / 1000),
      });
    }
  }, []);

  const loadHistory = useCallback(async () => {
    const generation = historyGeneration.current + 1;
    historyGeneration.current = generation;
    setHistoryLoading(true);
    try {
      const entries = unwrapCommandResult(
        await commands.getSendSelectedTextHistory(HISTORY_PAGE_SIZE + 1, 0),
      );
      if (historyGeneration.current !== generation) return;
      setHistoryEntries(entries.slice(0, HISTORY_PAGE_SIZE));
      setHistoryHasMore(entries.length > HISTORY_PAGE_SIZE);
    } catch (error) {
      if (historyGeneration.current === generation) {
        toast.error(t("sendSelectedText.messages.historyLoadFailed", { error: String(error) }));
      }
    } finally {
      if (historyGeneration.current === generation) {
        setHistoryLoading(false);
      }
    }
  }, [t]);

  const loadMoreHistory = async () => {
    if (
      historyClearInFlight.current ||
      historyLoading ||
      historyLoadingMore ||
      !historyHasMore
    ) {
      return;
    }
    const generation = historyGeneration.current;
    setHistoryLoadingMore(true);
    try {
      const entries = unwrapCommandResult(
        await commands.getSendSelectedTextHistory(
          HISTORY_PAGE_SIZE + 1,
          historyEntries.length,
        ),
      );
      if (historyGeneration.current !== generation) return;
      const page = entries.slice(0, HISTORY_PAGE_SIZE);
      setHistoryEntries((current) => {
        const knownIds = new Set(current.map((entry) => entry.id));
        return [...current, ...page.filter((entry) => !knownIds.has(entry.id))];
      });
      setHistoryHasMore(entries.length > HISTORY_PAGE_SIZE);
    } catch (error) {
      if (historyGeneration.current === generation) {
        toast.error(t("sendSelectedText.messages.olderHistoryLoadFailed", { error: String(error) }));
      }
    } finally {
      setHistoryLoadingMore(false);
    }
  };

  useEffect(() => {
    let active = true;
    let unlistenHistory: (() => void) | undefined;
    const initialize = async () => {
      try {
        const unlisten = await listen(
          "send-selected-text-history-updated",
          () => {
            void loadHistory();
          },
        );
        if (!active) {
          unlisten();
          return;
        }
        unlistenHistory = unlisten;
      } catch (error) {
        if (active) {
          toast.error(t("sendSelectedText.messages.historyWatchFailed", { error: String(error) }));
        }
      }
      if (!active) return;

      try {
        await Promise.all([loadFeature(), loadHistory()]);
      } catch (error) {
        if (active) toast.error(t("sendSelectedText.messages.settingsLoadFailed", { error: String(error) }));
      } finally {
        if (active) setLoading(false);
      }
    };
    void initialize();
    return () => {
      active = false;
      unlistenHistory?.();
    };
  }, [loadFeature, loadHistory, t]);

  const presets = feature?.presets ?? [];
  const { items: visiblePresets } =
    useSortedDisplayNames(presets, (preset) => preset.name, presetSortDirection);

  const createPreset = async () => {
    if (createPresetInFlight.current) return;
    createPresetInFlight.current = true;
    setCreatingPreset(true);
    try {
      const created = asCompletePreset(
        unwrapCommandResult(await commands.createSendSelectedTextPreset({
          id: "",
          name: t("sendSelectedText.presets.defaultName"),
        })),
      );
      featureGeneration.current += 1;
      setFeature((current) =>
        current
          ? { ...current, presets: [...current.presets, created] }
          : current,
      );
      await refreshSettings();
      toast.success(t("sendSelectedText.messages.presetCreated"));
    } catch (error) {
      toast.error(t("sendSelectedText.messages.presetCreateFailed", { error: String(error) }));
    } finally {
      createPresetInFlight.current = false;
      setCreatingPreset(false);
    }
  };

  const savePreset = async (
    preset: SendSelectedTextPreset,
  ): Promise<SendSelectedTextPreset> => {
    try {
      const updated = asCompletePreset(
        unwrapCommandResult(await commands.updateSendSelectedTextPreset(preset)),
      );
      featureGeneration.current += 1;
      setFeature((current) =>
        current
          ? {
              ...current,
              presets: current.presets.map((candidate) =>
                candidate.id === updated.id ? updated : candidate,
              ),
            }
          : current,
      );
      await refreshSettings();
      return updated;
    } catch (error) {
      toast.error(t("sendSelectedText.messages.presetSaveFailed", { error: String(error) }));
      throw error;
    }
  };

  const duplicatePreset = async (preset: SendSelectedTextPreset) => {
    try {
      const created = asCompletePreset(
        unwrapCommandResult(
          await commands.createSendSelectedTextPreset({
            ...preset,
            id: "",
            name: t("sendSelectedText.presets.copyName", { name: preset.name }),
          }),
        ),
      );
      featureGeneration.current += 1;
      setFeature((current) =>
        current
          ? { ...current, presets: [...current.presets, created] }
          : current,
      );
      await refreshSettings();
      toast.success(t("sendSelectedText.messages.presetDuplicated"));
    } catch (error) {
      toast.error(t("sendSelectedText.messages.presetDuplicateFailed", { error: String(error) }));
    }
  };

  const deletePreset = async (preset: SendSelectedTextPreset) => {
    if (
      !window.confirm(
        t("sendSelectedText.messages.deletePresetConfirm", { name: preset.name }),
      )
    ) {
      return;
    }
    try {
      unwrapCommandResult(await commands.deleteSendSelectedTextPreset(preset.id));
      featureGeneration.current += 1;
      setFeature((current) =>
        current
          ? {
              ...current,
              presets: current.presets.filter(
                (candidate) => candidate.id !== preset.id,
              ),
            }
          : current,
      );
      await refreshSettings();
      toast.success(t("sendSelectedText.messages.presetDeleted"));
    } catch (error) {
      toast.error(t("sendSelectedText.messages.presetDeleteFailed", { error: String(error) }));
    }
  };

  const runSample = async (preset: SendSelectedTextPreset, text: string) => {
    await savePreset(preset);
    try {
      const result = unwrapCommandResult(
        await commands.runSendSelectedTextPreset(preset.id, text),
      );
      toast.success(t("sendSelectedText.messages.sampleSaved", { path: result.output_path }));
      await loadHistory();
    } catch (error) {
      toast.error(t("sendSelectedText.messages.sampleFailed", { error: String(error) }));
      throw error;
    }
  };

  const trimJson = async (preset: SendSelectedTextPreset) => {
    await savePreset(preset);
    try {
      const removed = unwrapCommandResult(
        await commands.trimSendSelectedTextJson(preset.id),
      );
      toast.success(
        removed === 0
          ? t("sendSelectedText.messages.jsonRetentionSatisfied")
          : t("sendSelectedText.messages.oldEntriesRemoved", { count: removed }),
      );
    } catch (error) {
      toast.error(t("sendSelectedText.messages.jsonTrimFailed", { error: String(error) }));
      throw error;
    }
  };

  const saveOptions = async () => {
    if (optionsSaveInFlight.current) return;
    optionsSaveInFlight.current = true;
    setOptionsSaving(true);
    const revision = optionsRevision.current;
    try {
      const updated = asCompleteFeatureSettings(
        unwrapCommandResult(
          await commands.updateSendSelectedTextOptions(
            optionsDraft.historyLimit,
            optionsDraft.errorSeconds * 1000,
          ),
        ),
      );
      featureGeneration.current += 1;
      setFeature((current) =>
        current
          ? {
              ...current,
              history_limit: updated.history_limit,
              error_overlay_auto_hide_ms: updated.error_overlay_auto_hide_ms,
            }
          : updated,
      );
      if (optionsRevision.current === revision) {
        optionsDirty.current = false;
        setOptionsDraft({
          historyLimit: updated.history_limit,
          errorSeconds: Math.round(updated.error_overlay_auto_hide_ms / 1000),
        });
        toast.success(t("sendSelectedText.messages.optionsSaved"));
      } else {
        toast.success(t("sendSelectedText.messages.optionsSavedWithNewerEdits"));
      }
      await loadHistory();
    } catch (error) {
      toast.error(t("sendSelectedText.messages.optionsSaveFailed", { error: String(error) }));
    } finally {
      optionsSaveInFlight.current = false;
      setOptionsSaving(false);
    }
  };

  const deleteHistoryEntry = async (id: number) => {
    if (historyClearInFlight.current) return;
    try {
      unwrapCommandResult(await commands.deleteSendSelectedTextHistoryEntry(id));
      await loadHistory();
    } catch (error) {
      toast.error(t("sendSelectedText.messages.historyEntryDeleteFailed", { error: String(error) }));
    }
  };

  const clearHistory = async () => {
    if (historyClearInFlight.current) return;
    if (
      !window.confirm(
        t("sendSelectedText.messages.clearHistoryConfirm"),
      )
    ) {
      return;
    }
    historyClearInFlight.current = true;
    setHistoryClearing(true);
    try {
      unwrapCommandResult(await commands.clearSendSelectedTextHistory());
      await loadHistory();
    } catch (error) {
      toast.error(t("sendSelectedText.messages.historyClearFailed", { error: String(error) }));
    } finally {
      historyClearInFlight.current = false;
      setHistoryClearing(false);
    }
  };

  const refreshHistory = async () => {
    if (historyClearInFlight.current) return;
    await loadHistory();
  };

  const retryInitialLoad = async () => {
    setLoading(true);
    try {
      await Promise.all([loadFeature(), loadHistory()]);
    } catch (error) {
      toast.error(t("sendSelectedText.messages.settingsLoadFailed", { error: String(error) }));
    } finally {
      setLoading(false);
    }
  };

  if (loading) {
    return (
      <div className="sst-page sst-empty">{t("sendSelectedText.loading")}</div>
    );
  }

  if (!feature) {
    return (
      <div className="sst-page sst-empty tall">
        <AlertTriangle size={28} />
        <strong>{t("sendSelectedText.loadError")}</strong>
        <span>{t("sendSelectedText.loadErrorHelp")}</span>
        <Button variant="secondary" onClick={retryInitialLoad}>
          {t("sendSelectedText.actions.retry")}
        </Button>
      </div>
    );
  }

  return (
    <main className="sst-page">
      <header className="sst-page-header">
        <div className="sst-page-title">
          <Send size={22} />
          <div>
            <h1 id="settings-selected-text-presets" tabIndex={-1}>{t("sendSelectedText.title")}</h1>
            <p>
              {t("sendSelectedText.description")}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {tab === "presets" && (
            <NameSortControl
              direction={presetSortDirection}
              onChange={setPresetSortDirection}
              label={t("sendSelectedText.actions.sortPresets")}
            />
          )}
          <Button
            variant="primary"
            disabled={creatingPreset}
            onClick={createPreset}
          >
            <span className="flex items-center gap-2">
              <Plus size={16} /> {t("sendSelectedText.actions.addPreset")}
            </span>
          </Button>
        </div>
      </header>

      <nav className="sst-tabs" aria-label={t("sendSelectedText.tabs.label")}>
        {(
          [
            ["presets", Settings2, t("sendSelectedText.tabs.presets")],
            [
              "history",
              Clock3,
              t("sendSelectedText.tabs.history", {
                entries: `${historyEntries.length}${historyHasMore ? "+" : ""}`,
              }),
            ],
            ["help", Lightbulb, t("sendSelectedText.tabs.help")],
          ] as const
        ).map(([id, Icon, label]) => (
          <button
            type="button"
            key={id}
            id={`settings-selected-text-${id}-tab`}
            data-settings-search-reveal="true"
            className={tab === id ? "active" : ""}
            onClick={() => setTab(id)}
          >
            <Icon size={15} />
            <span>{label}</span>
          </button>
        ))}
      </nav>

      {tab === "presets" && (
        <div className="space-y-4">
          <section className="sst-options-bar">
            <div>
              <h2 id="settings-selected-text-history-options" tabIndex={-1}>{t("sendSelectedText.options.title")}</h2>
              <p>{t("sendSelectedText.options.description")}</p>
            </div>
            <label>
              <span>{t("sendSelectedText.options.historyLimit")}</span>
              <Input
                type="number"
                variant="compact"
                className="w-full text-xs"
                min={1}
                max={5000}
                value={optionsDraft.historyLimit}
                onChange={(event) => {
                  optionsDirty.current = true;
                  optionsRevision.current += 1;
                  setOptionsDraft((current) => ({
                    ...current,
                    historyLimit: Number(event.target.value),
                  }));
                }}
              />
            </label>
            <label>
              <span>{t("sendSelectedText.options.errorSeconds")}</span>
              <Input
                type="number"
                variant="compact"
                className="w-full text-xs"
                min={1}
                max={100}
                value={optionsDraft.errorSeconds}
                onChange={(event) => {
                  optionsDirty.current = true;
                  optionsRevision.current += 1;
                  setOptionsDraft((current) => ({
                    ...current,
                    errorSeconds: Number(event.target.value),
                  }));
                }}
              />
            </label>
            <Button
              type="button"
              variant="secondary"
              size="sm"
              className="flex min-h-[34px] items-center justify-center gap-2 whitespace-nowrap"
              disabled={optionsSaving}
              onClick={saveOptions}
              title={t("sendSelectedText.actions.saveOptions")}
            >
              <Save size={15} />
              <span>{t("sendSelectedText.actions.save")}</span>
            </Button>
          </section>

          {presets.length === 0 ? (
            <div className="sst-empty tall">
              <Send size={28} />
              <strong>{t("sendSelectedText.presets.empty")}</strong>
              <span>
                {t("sendSelectedText.presets.emptyHelp")}
              </span>
              <Button disabled={creatingPreset} onClick={createPreset}>
                <span className="flex items-center gap-2">
                  <Plus size={16} /> {t("sendSelectedText.actions.addFirstPreset")}
                </span>
              </Button>
            </div>
          ) : (
            <div className="space-y-3">
              {visiblePresets.map((preset) => (
                <PresetCard
                  key={preset.id}
                  preset={preset}
                  onSave={savePreset}
                  onDelete={deletePreset}
                  onDuplicate={duplicatePreset}
                  onRunSample={runSample}
                  onTrimJson={trimJson}
                />
              ))}
            </div>
          )}
        </div>
      )}

      {tab === "history" && (
        <HistoryView
          entries={historyEntries}
          loading={historyLoading}
          loadingMore={historyLoadingMore}
          clearing={historyClearing}
          hasMore={historyHasMore}
          onRefresh={refreshHistory}
          onLoadMore={loadMoreHistory}
          onDelete={deleteHistoryEntry}
          onClear={clearHistory}
        />
      )}

      {tab === "help" && <HelpView />}
    </main>
  );
}
