import React, { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
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
import { useSettings } from "@/hooks/useSettings";
import { sessionToast as toast } from "@/lib/sessionToast";
import { Button } from "../../ui/Button";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { HandyShortcut } from "../HandyShortcut";
import "./SendSelectedTextSettings.css";

type OutputFormat = "markdown" | "json";
type WriteMode =
  | "create_new"
  | "append_last"
  | "append_file"
  | "overwrite_file";
type CaptureMode = "auto" | "clipboard_copy" | "accessibility";
type OversizeBehavior = "reject" | "truncate";
type ExecutionPolicy = "default" | "bypass" | "unrestricted" | "remote_signed";
type HistoryStatus =
  | "saved"
  | "command_started"
  | "completed"
  | "command_failed"
  | "failed";

interface SendSelectedTextPreset {
  id: string;
  name: string;
  enabled: boolean;
  format: OutputFormat;
  write_mode: WriteMode;
  capture_mode: CaptureMode;
  destination_directory: string;
  filename_template: string;
  content_template: string;
  max_chars: number;
  oversize_behavior: OversizeBehavior;
  json_keep_last: number;
  command_enabled: boolean;
  command: string;
  allow_text_variable: boolean;
  command_silent: boolean;
  command_no_profile: boolean;
  command_use_pwsh: boolean;
  command_execution_policy: ExecutionPolicy;
  command_working_directory: string;
}

interface SendSelectedTextFeatureSettings {
  presets: SendSelectedTextPreset[];
  history_limit: number;
  error_overlay_auto_hide_ms: number;
}

interface SendSelectedTextHistoryEntry {
  id: number;
  operation_id: string;
  preset_id: string;
  preset_name: string;
  timestamp_ms: number;
  selected_text: string;
  output_path: string | null;
  output_format: string;
  write_mode: string;
  status: HistoryStatus;
  command: string | null;
  command_output: string | null;
  command_output_truncated: boolean;
  error: string | null;
}

interface OperationResult {
  history_id: number;
  operation_id: string;
  output_path: string;
  status: HistoryStatus;
  command_output: string | null;
  command_output_truncated: boolean;
}

type PageTab = "presets" | "history" | "help";

const STATUS_LABELS: Record<HistoryStatus, string> = {
  saved: "Saved",
  command_started: "Command started",
  completed: "Completed",
  command_failed: "Command failed",
  failed: "Failed",
};

const WRITE_MODE_LABELS: Record<WriteMode, string> = {
  create_new: "Create a new file",
  append_last: "Append to this preset's last file",
  append_file: "Append to the rendered filename",
  overwrite_file: "Overwrite the rendered filename",
};

const COPY_EXAMPLES = [
  {
    title: "One Markdown file per selection",
    summary: "A clean inbox where every capture remains independent.",
    fields: [
      ["Format", "Markdown"],
      ["Action", "Create a new file"],
      ["Filename", "selected-{{date}}-{{time}}.md"],
      ["Content", "{{text}}"],
    ],
  },
  {
    title: "Append to a daily Markdown note",
    summary: "Collect snippets in one file per day.",
    fields: [
      ["Format", "Markdown"],
      ["Action", "Append to the rendered filename"],
      ["Filename", "inbox-{{date}}.md"],
      ["Content", "## {{timestamp_local}}\n\n{{text}}"],
    ],
  },
  {
    title: "Continue the last task file",
    summary:
      "The preset remembers its latest successful output even if History is cleared.",
    fields: [
      ["Format", "Markdown"],
      ["Action", "Append to this preset's last file"],
      ["Filename", "task-{{date}}-{{time}}.md"],
      ["Content", "{{text}}"],
    ],
  },
  {
    title: "Rolling JSON inbox",
    summary: "One valid JSON document that keeps only its newest 50 records.",
    fields: [
      ["Format", "JSON"],
      ["Action", "Append to the rendered filename"],
      ["Filename", "selected-text.json"],
      ["Keep latest", "50"],
    ],
  },
];

const COMMAND_EXAMPLES = [
  {
    title: "Ask Codex to explain or solve the selected task",
    description: "Reads the UTF-8 input file and gives Codex read-only access.",
    command:
      "Get-Content -Raw -Encoding UTF8 -LiteralPath {{input_file}} | codex exec -C {{working_directory}} -s read-only -",
  },
  {
    title: "Ask Codex to implement selected tasks",
    description: "Use a repository as the command working directory.",
    command:
      "Get-Content -Raw -Encoding UTF8 -LiteralPath {{input_file}} | codex exec -C {{working_directory}} -s workspace-write -",
  },
  {
    title: "Process selected text",
    description:
      "The saved text becomes stdin; the prompt describes the transformation.",
    command:
      '("Process the supplied text. Return only the useful final result.`n`n" + (Get-Content -Raw -Encoding UTF8 -LiteralPath {{input_file}})) | codex exec -s read-only -',
  },
  {
    title: "Remove unwanted material",
    description: "A starting point for cleanup workflows.",
    command:
      '("Remove repetition, boilerplate, and irrelevant material. Return only the cleaned text.`n`n" + (Get-Content -Raw -Encoding UTF8 -LiteralPath {{input_file}})) | codex exec -s read-only -',
  },
  {
    title: "Summarize as Markdown",
    description: "Produces concise bullets from the selected source.",
    command:
      '("Summarize the supplied text as concise Markdown bullets.`n`n" + (Get-Content -Raw -Encoding UTF8 -LiteralPath {{input_file}})) | codex exec -s read-only -',
  },
  {
    title: "Let Codex maintain the saved inbox",
    description:
      "Passes the destination file path instead of command-line text.",
    command:
      'codex exec -C {{working_directory}} -s workspace-write ("Review the inbox file at " + {{file_path}} + ". Organize it, preserve useful content, and remove duplicates.")',
  },
  {
    title: "Run any file-aware agent",
    description: "The same variables work with another installed CLI.",
    command: "my-agent --input-file {{input_file}} --output-file {{file_path}}",
  },
  {
    title: "Direct text insertion",
    description:
      "Enable direct {{text}} insertion first; file input is safer for long text.",
    command: 'codex exec -s read-only ("Solve this task: " + {{text}})',
  },
];

const VARIABLES = [
  ["{{input_file}}", "Temporary UTF-8 file containing only this selection"],
  ["{{file_path}}", "The successfully saved Markdown or JSON file"],
  ["{{directory}}", "Destination directory"],
  ["{{filename}}", "Destination filename"],
  ["{{text}}", "Selected text; requires the direct insertion toggle"],
  ["{{record_id}}", "Stable ID for this capture"],
  ["{{timestamp}}", "UTC timestamp"],
  ["{{timestamp_local}}", "Timestamp in the computer's time zone"],
  ["{{date}} / {{time}}", "Filename-safe date and time"],
  ["{{preset_id}} / {{preset_name}}", "Preset identity"],
  ["{{format}} / {{write_mode}}", "Actual output strategy"],
  ["{{text_length}}", "Selected-text character count"],
  ["{{working_directory}}", "The command working-directory field"],
];

const formatDate = (timestampMs: number) =>
  new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "medium",
  }).format(new Date(timestampMs));

const previewText = (value: string, maximum = 220) => {
  const compact = value.replace(/\s+/g, " ").trim();
  return compact.length > maximum ? `${compact.slice(0, maximum)}...` : compact;
};

function FieldLabel({ children }: { children: React.ReactNode }) {
  return <span className="sst-field-label">{children}</span>;
}

function CopyButton({ value, label }: { value: string; label: string }) {
  const [copied, setCopied] = useState(false);
  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1400);
    } catch (error) {
      toast.error(`Could not copy ${label}: ${String(error)}`);
    }
  };
  return (
    <button
      type="button"
      className="sst-icon-button"
      onClick={handleCopy}
      title={`Copy ${label}`}
      aria-label={`Copy ${label}`}
    >
      {copied ? <Check size={15} /> : <Copy size={15} />}
    </button>
  );
}

interface PresetCardProps {
  preset: SendSelectedTextPreset;
  onSave: (preset: SendSelectedTextPreset) => Promise<void>;
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
  const [draft, setDraft] = useState(preset);
  const [expanded, setExpanded] = useState(true);
  const [busy, setBusy] = useState(false);
  const [sampleText, setSampleText] = useState(
    "A sample selection saved by AivoRelay.",
  );

  useEffect(() => setDraft(preset), [preset]);

  const update = <K extends keyof SendSelectedTextPreset>(
    key: K,
    value: SendSelectedTextPreset[K],
  ) => setDraft((current) => ({ ...current, [key]: value }));

  const saveDraft = async () => {
    setBusy(true);
    try {
      await onSave(draft);
    } catch {
      // The parent reports the save error.
    } finally {
      setBusy(false);
    }
  };

  const runSampleDraft = async () => {
    setBusy(true);
    try {
      await onRunSample(draft, sampleText);
    } finally {
      setBusy(false);
    }
  };

  const trimJsonDraft = async () => {
    setBusy(true);
    try {
      await onTrimJson(draft);
    } finally {
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
              <span className="sst-status-chip neutral">Disabled</span>
            )}
          </div>
          <p className="mt-1 truncate text-xs text-[#969696]">
            {WRITE_MODE_LABELS[preset.write_mode]}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <button
            type="button"
            className="sst-icon-button"
            onClick={() => onDuplicate(preset)}
            title="Duplicate preset"
            aria-label="Duplicate preset"
          >
            <ClipboardCopy size={15} />
          </button>
          <button
            type="button"
            className="sst-icon-button danger"
            onClick={() => onDelete(preset)}
            title="Delete preset"
            aria-label="Delete preset"
          >
            <Trash2 size={15} />
          </button>
          <button
            type="button"
            className="sst-icon-button"
            onClick={() => setExpanded((value) => !value)}
            title={expanded ? "Collapse preset" : "Expand preset"}
            aria-label={expanded ? "Collapse preset" : "Expand preset"}
            aria-expanded={expanded}
          >
            {expanded ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
          </button>
        </div>
      </header>

      {expanded && (
        <div className="sst-preset-body">
          <div className="sst-form-grid two">
            <label className="sst-field">
              <FieldLabel>Preset name</FieldLabel>
              <input
                value={draft.name}
                onChange={(event) => update("name", event.target.value)}
              />
            </label>
            <div className="sst-field">
              <FieldLabel>Enabled</FieldLabel>
              <span className="sst-toggle-row">
                <ToggleSwitch
                  checked={draft.enabled}
                  onChange={(checked) => update("enabled", checked)}
                  ariaLabel={`Enable ${draft.name}`}
                />
                <span>Hotkey and preset may run</span>
              </span>
            </div>
          </div>

          <div className="sst-hotkey-row">
            <div>
              <FieldLabel>Preset hotkey</FieldLabel>
              <p>
                Captures after the keys are released, so Ctrl+C remains
                reliable.
              </p>
            </div>
            <HandyShortcut
              shortcutId={`send_selected_text_${preset.id}`}
              grouped
              descriptionMode="tooltip"
              disabled={!draft.enabled}
            />
          </div>

          <div className="sst-form-grid three">
            <label className="sst-field">
              <FieldLabel>Format</FieldLabel>
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
              <FieldLabel>File action</FieldLabel>
              <select
                value={draft.write_mode}
                onChange={(event) =>
                  update("write_mode", event.target.value as WriteMode)
                }
              >
                {writeModes.map((mode) => (
                  <option key={mode} value={mode}>
                    {WRITE_MODE_LABELS[mode]}
                  </option>
                ))}
              </select>
            </label>
          </div>

          <div className="sst-field">
            <FieldLabel>Destination folder</FieldLabel>
            <div className="sst-input-action-row">
              <input
                value={draft.destination_directory}
                onChange={(event) =>
                  update("destination_directory", event.target.value)
                }
                placeholder="Choose where this preset writes files"
              />
              <button
                type="button"
                className="sst-icon-button wide"
                onClick={chooseDirectory}
                title="Choose folder"
              >
                <FolderOpen size={16} />
                <span>Browse</span>
              </button>
            </div>
          </div>

          <label className="sst-field">
            <FieldLabel>Filename template</FieldLabel>
            <input
              className="font-mono"
              value={draft.filename_template}
              onChange={(event) =>
                update("filename_template", event.target.value)
              }
            />
            <small>
              Filename variables: {"{{date}}"}, {"{{time}}"}, {"{{record_id}}"},{" "}
              {"{{preset_name}}"}. Folder separators are rejected.
            </small>
          </label>

          {draft.format === "markdown" ? (
            <label className="sst-field">
              <FieldLabel>Markdown content template</FieldLabel>
              <textarea
                className="min-h-[104px] font-mono"
                value={draft.content_template}
                onChange={(event) =>
                  update("content_template", event.target.value)
                }
              />
              <small>
                {"{{text}}"} is required. Optional: {"{{timestamp_local}}"},{" "}
                {"{{preset_name}}"}, {"{{record_id}}"}.
              </small>
            </label>
          ) : (
            <div className="sst-json-note">
              <FileJson size={18} />
              <div>
                <strong>JSON is generated automatically</strong>
                <p>
                  AivoRelay writes one versioned document with an entries array.
                  Quotes, newlines, backslashes, and Unicode are escaped by the
                  JSON serializer.
                </p>
              </div>
              <label>
                <span>Keep latest entries</span>
                <input
                  type="number"
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
                    Trim existing JSON now
                  </Button>
                )}
            </div>
          )}

          <div className="sst-form-grid three">
            <label className="sst-field">
              <FieldLabel>Capture method</FieldLabel>
              <select
                value={draft.capture_mode}
                onChange={(event) =>
                  update("capture_mode", event.target.value as CaptureMode)
                }
              >
                <option value="auto">Auto: accessibility, then copy</option>
                <option value="clipboard_copy">
                  Copy and restore clipboard
                </option>
                <option value="accessibility">
                  Windows accessibility only
                </option>
              </select>
            </label>
            <label className="sst-field">
              <FieldLabel>Maximum characters</FieldLabel>
              <input
                type="number"
                min={1}
                max={2000000}
                value={draft.max_chars}
                onChange={(event) =>
                  update("max_chars", Number(event.target.value))
                }
              />
            </label>
            <label className="sst-field">
              <FieldLabel>If text is too long</FieldLabel>
              <select
                value={draft.oversize_behavior}
                onChange={(event) =>
                  update(
                    "oversize_behavior",
                    event.target.value as OversizeBehavior,
                  )
                }
              >
                <option value="reject">Reject without saving</option>
                <option value="truncate">Save the allowed beginning</option>
              </select>
            </label>
          </div>

          <section className="sst-command-section">
            <div className="sst-section-heading-row">
              <div>
                <h4>Run a command after saving</h4>
                <p>The file remains saved even if this command fails.</p>
              </div>
              <label className="sst-toggle-row compact">
                <input
                  type="checkbox"
                  checked={draft.command_enabled}
                  onChange={(event) =>
                    update("command_enabled", event.target.checked)
                  }
                />
                <span>Enabled</span>
              </label>
            </div>
            {draft.command_enabled && (
              <div className="space-y-3">
                <label className="sst-field">
                  <FieldLabel>PowerShell command</FieldLabel>
                  <textarea
                    className="min-h-[112px] font-mono"
                    value={draft.command}
                    onChange={(event) => update("command", event.target.value)}
                    placeholder="Get-Content -Raw -Encoding UTF8 -LiteralPath {{input_file}} | codex exec -s read-only -"
                  />
                  <small>
                    Write placeholders without surrounding quotes. AivoRelay
                    inserts them as PowerShell single-quoted literals.
                  </small>
                </label>
                <div className="sst-form-grid two">
                  <label className="sst-field">
                    <FieldLabel>Working directory</FieldLabel>
                    <input
                      value={draft.command_working_directory}
                      onChange={(event) =>
                        update("command_working_directory", event.target.value)
                      }
                      placeholder="Optional project directory"
                    />
                  </label>
                  <label className="sst-field">
                    <FieldLabel>Execution policy</FieldLabel>
                    <select
                      value={draft.command_execution_policy}
                      onChange={(event) =>
                        update(
                          "command_execution_policy",
                          event.target.value as ExecutionPolicy,
                        )
                      }
                    >
                      <option value="default">System default</option>
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
                    Capture exit code and output in History
                  </label>
                  <label>
                    <input
                      type="checkbox"
                      checked={draft.command_no_profile}
                      onChange={(event) =>
                        update("command_no_profile", event.target.checked)
                      }
                    />
                    Skip PowerShell profile
                  </label>
                  <label>
                    <input
                      type="checkbox"
                      checked={draft.command_use_pwsh}
                      onChange={(event) =>
                        update("command_use_pwsh", event.target.checked)
                      }
                    />
                    Use PowerShell 7 (pwsh)
                  </label>
                  <label>
                    <input
                      type="checkbox"
                      checked={draft.allow_text_variable}
                      onChange={(event) =>
                        update("allow_text_variable", event.target.checked)
                      }
                    />
                    Allow direct {"{{text}}"} insertion
                  </label>
                </div>
                {draft.allow_text_variable && (
                  <div className="sst-warning-row">
                    <AlertTriangle size={17} />
                    <span>
                      Direct text is safely PowerShell-quoted, but long
                      selections can still exceed Windows command-line limits.
                      Prefer {"{{input_file}}"}.
                    </span>
                  </div>
                )}
              </div>
            )}
          </section>

          <section className="sst-sample-section">
            <div>
              <FieldLabel>Test without changing the clipboard</FieldLabel>
              <p>
                Runs this preset with sample text instead of the current
                selection.
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
                <Play size={14} /> Test preset
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
                <Save size={15} /> Save preset
              </span>
            </Button>
          </footer>
        </div>
      )}
    </article>
  );
}

function HistoryView({
  entries,
  loading,
  onRefresh,
  onDelete,
  onClear,
}: {
  entries: SendSelectedTextHistoryEntry[];
  loading: boolean;
  onRefresh: () => Promise<void>;
  onDelete: (id: number) => Promise<void>;
  onClear: () => Promise<void>;
}) {
  return (
    <div className="space-y-3">
      <div className="sst-toolbar">
        <div>
          <h2>Execution history</h2>
          <p>Selected text, saved path, command output, and complete errors.</p>
        </div>
        <div className="flex gap-2">
          <Button variant="secondary" size="sm" onClick={onRefresh}>
            Refresh
          </Button>
          <Button
            variant="danger"
            size="sm"
            disabled={entries.length === 0}
            onClick={onClear}
          >
            Clear history
          </Button>
        </div>
      </div>
      {loading ? (
        <div className="sst-empty">Loading history...</div>
      ) : entries.length === 0 ? (
        <div className="sst-empty">
          <History size={22} />
          <span>No Send Selected Text runs yet.</span>
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
                      {STATUS_LABELS[entry.status]}
                    </span>
                    <span className="sst-history-format">
                      {entry.output_format}
                    </span>
                  </div>
                  <time>{formatDate(entry.timestamp_ms)}</time>
                </div>
                <button
                  type="button"
                  className="sst-icon-button danger"
                  onClick={() => onDelete(entry.id)}
                  title="Delete history entry"
                  aria-label="Delete history entry"
                >
                  <Trash2 size={15} />
                </button>
              </header>
              <div className="sst-history-block">
                <div className="sst-history-block-title">
                  <span>Selected text</span>
                  <CopyButton
                    value={entry.selected_text}
                    label="selected text"
                  />
                </div>
                <p>{previewText(entry.selected_text) || "No text captured"}</p>
              </div>
              {entry.output_path && (
                <div className="sst-history-path">
                  <span>{entry.output_path}</span>
                  <CopyButton value={entry.output_path} label="file path" />
                </div>
              )}
              {entry.command && (
                <details className="sst-history-details">
                  <summary>Command</summary>
                  <div className="sst-history-detail-content">
                    <pre>{entry.command}</pre>
                    <CopyButton value={entry.command} label="command" />
                  </div>
                </details>
              )}
              {entry.command_output && (
                <details className="sst-history-details">
                  <summary>Command output</summary>
                  <div className="sst-history-detail-content">
                    <pre>{entry.command_output}</pre>
                    <CopyButton
                      value={entry.command_output}
                      label="command output"
                    />
                  </div>
                  {entry.command_output_truncated && (
                    <small>
                      Output was truncated at the configured storage boundary.
                    </small>
                  )}
                </details>
              )}
              {entry.error && (
                <div className="sst-history-error">
                  <div className="sst-history-block-title">
                    <span>Full error</span>
                    <CopyButton value={entry.error} label="full error" />
                  </div>
                  <pre>{entry.error}</pre>
                </div>
              )}
            </article>
          ))}
        </div>
      )}
    </div>
  );
}

function HelpView() {
  return (
    <div className="space-y-5">
      <section className="sst-help-intro">
        <Lightbulb size={21} />
        <div>
          <h2>How this workflow fits together</h2>
          <ol>
            <li>Select text in any application.</li>
            <li>Press the hotkey assigned to a preset.</li>
            <li>AivoRelay reads the selection and restores your clipboard.</li>
            <li>The preset writes Markdown or a structured JSON record.</li>
            <li>An optional command receives safe file paths and variables.</li>
            <li>
              History records the text, path, output, and any complete error.
            </li>
          </ol>
        </div>
      </section>

      <section>
        <div className="sst-help-heading">
          <FileText size={18} />
          <div>
            <h2>File recipes</h2>
            <p>Use these field combinations as starting points.</p>
          </div>
        </div>
        <div className="sst-example-grid">
          {COPY_EXAMPLES.map((example) => (
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
            <h2>Codex CLI and command examples</h2>
            <p>
              These are PowerShell commands. Set Working directory to the
              project that Codex should inspect or modify.
            </p>
          </div>
        </div>
        <div className="space-y-2">
          {COMMAND_EXAMPLES.map((example) => (
            <article key={example.title} className="sst-command-example">
              <div>
                <h3>{example.title}</h3>
                <p>{example.description}</p>
              </div>
              <pre>{example.command}</pre>
              <CopyButton value={example.command} label="command" />
            </article>
          ))}
        </div>
      </section>

      <section>
        <div className="sst-help-heading">
          <Settings2 size={18} />
          <div>
            <h2>Variables</h2>
            <p>Placeholders are replaced only after the file has been saved.</p>
          </div>
        </div>
        <div className="sst-variable-table">
          {VARIABLES.map(([name, description]) => (
            <React.Fragment key={name}>
              <code>{name}</code>
              <span>{description}</span>
              <CopyButton value={name} label="variable" />
            </React.Fragment>
          ))}
        </div>
      </section>

      <section className="sst-safety-help">
        <AlertTriangle size={20} />
        <div>
          <h2>Command and privacy notes</h2>
          <p>
            Commands are authored by you and run with your Windows account. Use
            captured execution when you need an exit code and logs.
            Visible-window commands are started without waiting, so their
            temporary input file is retained and cleaned after it becomes stale.
          </p>
          <p>
            History contains the selected text. Set a suitable history limit and
            clear it when the source is sensitive. JSON escaping is automatic;
            never add manual backslashes merely to make ordinary text valid
            JSON.
          </p>
        </div>
      </section>
    </div>
  );
}

export default function SendSelectedTextSettings() {
  const { refreshSettings } = useSettings();
  const [tab, setTab] = useState<PageTab>("presets");
  const [feature, setFeature] =
    useState<SendSelectedTextFeatureSettings | null>(null);
  const [historyEntries, setHistoryEntries] = useState<
    SendSelectedTextHistoryEntry[]
  >([]);
  const [loading, setLoading] = useState(true);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [optionsDraft, setOptionsDraft] = useState({
    historyLimit: 200,
    errorSeconds: 10,
  });

  const loadFeature = useCallback(async () => {
    const value = await invoke<SendSelectedTextFeatureSettings>(
      "get_send_selected_text_settings",
    );
    setFeature(value);
    setOptionsDraft({
      historyLimit: value.history_limit,
      errorSeconds: Math.round(value.error_overlay_auto_hide_ms / 1000),
    });
  }, []);

  const loadHistory = useCallback(async () => {
    setHistoryLoading(true);
    try {
      setHistoryEntries(
        await invoke<SendSelectedTextHistoryEntry[]>(
          "get_send_selected_text_history",
          { limit: 500, offset: 0 },
        ),
      );
    } finally {
      setHistoryLoading(false);
    }
  }, []);

  useEffect(() => {
    let active = true;
    Promise.all([loadFeature(), loadHistory()])
      .catch((error) => {
        if (active) toast.error(String(error));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    const unlistenPromise = listen("send-selected-text-history-updated", () => {
      void loadHistory();
    });
    return () => {
      active = false;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [loadFeature, loadHistory]);

  const presets = feature?.presets ?? [];
  const sortedPresets = useMemo(() => presets, [presets]);

  const createPreset = async () => {
    try {
      await invoke<SendSelectedTextPreset>("create_send_selected_text_preset", {
        template: null,
      });
      await Promise.all([loadFeature(), refreshSettings()]);
      toast.success("Preset created");
    } catch (error) {
      toast.error(String(error));
    }
  };

  const savePreset = async (preset: SendSelectedTextPreset) => {
    try {
      await invoke<SendSelectedTextPreset>("update_send_selected_text_preset", {
        preset,
      });
      await Promise.all([loadFeature(), refreshSettings()]);
      toast.success("Preset saved");
    } catch (error) {
      toast.error(String(error));
      throw error;
    }
  };

  const duplicatePreset = async (preset: SendSelectedTextPreset) => {
    try {
      await invoke<SendSelectedTextPreset>("create_send_selected_text_preset", {
        template: { ...preset, id: "", name: `${preset.name} copy` },
      });
      await Promise.all([loadFeature(), refreshSettings()]);
      toast.success("Preset duplicated without copying its hotkey");
    } catch (error) {
      toast.error(String(error));
    }
  };

  const deletePreset = async (preset: SendSelectedTextPreset) => {
    if (
      !window.confirm(
        `Delete preset "${preset.name}"? Its history will remain.`,
      )
    ) {
      return;
    }
    try {
      await invoke("delete_send_selected_text_preset", { presetId: preset.id });
      await Promise.all([loadFeature(), refreshSettings()]);
      toast.success("Preset deleted; history preserved");
    } catch (error) {
      toast.error(String(error));
    }
  };

  const runSample = async (preset: SendSelectedTextPreset, text: string) => {
    try {
      await savePreset(preset);
      const result = await invoke<OperationResult>(
        "run_send_selected_text_preset",
        {
          presetId: preset.id,
          sampleText: text,
        },
      );
      toast.success(`Saved to ${result.output_path}`);
      await loadHistory();
    } catch (error) {
      toast.error(String(error));
    }
  };

  const trimJson = async (preset: SendSelectedTextPreset) => {
    try {
      await savePreset(preset);
      const removed = await invoke<number>("trim_send_selected_text_json", {
        presetId: preset.id,
      });
      toast.success(
        removed === 0
          ? "JSON already satisfies retention"
          : `Removed ${removed} old entries`,
      );
    } catch (error) {
      toast.error(String(error));
    }
  };

  const saveOptions = async () => {
    try {
      const updated = await invoke<SendSelectedTextFeatureSettings>(
        "update_send_selected_text_options",
        {
          historyLimit: optionsDraft.historyLimit,
          errorOverlayAutoHideMs: optionsDraft.errorSeconds * 1000,
        },
      );
      setFeature(updated);
      setOptionsDraft({
        historyLimit: updated.history_limit,
        errorSeconds: Math.round(updated.error_overlay_auto_hide_ms / 1000),
      });
      toast.success("History and overlay settings saved");
      await loadHistory();
    } catch (error) {
      toast.error(String(error));
    }
  };

  const deleteHistoryEntry = async (id: number) => {
    try {
      await invoke("delete_send_selected_text_history_entry", { id });
      await loadHistory();
    } catch (error) {
      toast.error(String(error));
    }
  };

  const clearHistory = async () => {
    if (
      !window.confirm(
        "Clear all Send Selected Text history? Saved output files remain.",
      )
    ) {
      return;
    }
    try {
      await invoke("clear_send_selected_text_history");
      await loadHistory();
    } catch (error) {
      toast.error(String(error));
    }
  };

  if (loading || !feature) {
    return (
      <div className="sst-page sst-empty">Loading Send Selected Text...</div>
    );
  }

  return (
    <main className="sst-page">
      <header className="sst-page-header">
        <div className="sst-page-title">
          <Send size={22} />
          <div>
            <h1>Send selected text to file or command</h1>
            <p>
              Save a selection to Markdown or JSON, then optionally run your
              command.
            </p>
          </div>
        </div>
        <Button variant="primary" onClick={createPreset}>
          <span className="flex items-center gap-2">
            <Plus size={16} /> Add preset
          </span>
        </Button>
      </header>

      <nav className="sst-tabs" aria-label="Send Selected Text views">
        {(
          [
            ["presets", Settings2, "Presets"],
            ["history", Clock3, `History (${historyEntries.length})`],
            ["help", Lightbulb, "Help / Examples"],
          ] as const
        ).map(([id, Icon, label]) => (
          <button
            type="button"
            key={id}
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
              <h2>History and error overlay</h2>
              <p>These settings apply to every preset.</p>
            </div>
            <label>
              <span>History entries</span>
              <input
                type="number"
                min={1}
                max={5000}
                value={optionsDraft.historyLimit}
                onChange={(event) =>
                  setOptionsDraft((current) => ({
                    ...current,
                    historyLimit: Number(event.target.value),
                  }))
                }
              />
            </label>
            <label>
              <span>Error overlay seconds</span>
              <input
                type="number"
                min={1}
                max={100}
                value={optionsDraft.errorSeconds}
                onChange={(event) =>
                  setOptionsDraft((current) => ({
                    ...current,
                    errorSeconds: Number(event.target.value),
                  }))
                }
              />
            </label>
            <button
              type="button"
              className="sst-icon-button wide"
              onClick={saveOptions}
              title="Save history and overlay settings"
            >
              <Save size={15} />
              <span>Save</span>
            </button>
          </section>

          {sortedPresets.length === 0 ? (
            <div className="sst-empty tall">
              <Send size={28} />
              <strong>No presets yet</strong>
              <span>
                Add a preset, choose its folder, then assign a hotkey.
              </span>
              <Button onClick={createPreset}>
                <span className="flex items-center gap-2">
                  <Plus size={16} /> Add first preset
                </span>
              </Button>
            </div>
          ) : (
            <div className="space-y-3">
              {sortedPresets.map((preset) => (
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
          onRefresh={loadHistory}
          onDelete={deleteHistoryEntry}
          onClear={clearHistory}
        />
      )}

      {tab === "help" && <HelpView />}
    </main>
  );
}
