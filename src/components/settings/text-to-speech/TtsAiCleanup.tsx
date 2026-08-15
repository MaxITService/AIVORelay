import { Loader2, Plus, RefreshCcw, Trash2 } from "lucide-react";
import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";

import { commands } from "@/bindings";
import { ApiKeyEditor, StoredApiKeyDisplay } from "../ApiKeyControls";
import { ModelSelect } from "../PostProcessingSettingsApi/ModelSelect";
import { TtsHelpDisclosure } from "./TtsHelpDisclosure";
import { CommittedNumberInput } from "./CommittedNumberInput";
import { Button } from "@/components/ui/Button";
import { ConfirmationModal } from "@/components/ui/ConfirmationModal";
import { Input } from "@/components/ui/Input";
import { Select, type SelectOption } from "@/components/ui/Select";
import { SettingContainer } from "@/components/ui/SettingContainer";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { Textarea } from "@/components/ui/Textarea";
import { ToggleSwitch } from "@/components/ui/ToggleSwitch";
import { Tooltip } from "@/components/ui/Tooltip";

export type TtsLlmPrompt = {
  id: string;
  name: string;
  prompt: string;
};

export type TtsLlmBenchmarkResult = {
  timestamp_ms: number;
  provider_id: string;
  provider_label: string;
  model: string;
  duration_ms: number;
  chars_per_second: number;
  input_chars: number;
  output_chars: number;
  success: boolean;
  system_prompt: string;
  user_message: string;
  response_text: string;
  error?: string | null;
};

export type TtsLlmPreprocessingSettings = {
  interactive_enabled: boolean;
  file_enabled: boolean;
  provider_id: string;
  model: string;
  key_source: "shared" | "separate";
  custom_base_url: string;
  custom_allow_insecure_http: boolean;
  reasoning_enabled: boolean;
  reasoning_budget: number;
  chunk_target_chars: number;
  retry_count: number;
  retry_base_delay_ms: number;
  request_timeout_seconds: number;
  interactive_prompts: TtsLlmPrompt[];
  interactive_selected_prompt_id: string;
  file_prompts: TtsLlmPrompt[];
  file_selected_prompt_id: string;
  section_collapsed: boolean;
  benchmark_collapsed: boolean;
  interactive_benchmark_text: string;
  file_benchmark_text: string;
  interactive_benchmark_log: TtsLlmBenchmarkResult[];
  file_benchmark_log: TtsLlmBenchmarkResult[];
};

export const DEFAULT_TTS_LLM_PREPROCESSING: TtsLlmPreprocessingSettings = {
  interactive_enabled: false,
  file_enabled: false,
  provider_id: "openrouter",
  model: "",
  key_source: "shared",
  custom_base_url: "",
  custom_allow_insecure_http: false,
  reasoning_enabled: false,
  reasoning_budget: 4096,
  chunk_target_chars: 12000,
  retry_count: 2,
  retry_base_delay_ms: 750,
  request_timeout_seconds: 120,
  interactive_prompts: [],
  interactive_selected_prompt_id: "",
  file_prompts: [],
  file_selected_prompt_id: "",
  section_collapsed: true,
  benchmark_collapsed: true,
  interactive_benchmark_text: "",
  file_benchmark_text: "",
  interactive_benchmark_log: [],
  file_benchmark_log: [],
};

type LlmProvider = {
  id: string;
  label: string;
};

type TtsAiCleanupProps = {
  mode: "interactive" | "files";
  value: TtsLlmPreprocessingSettings;
  providers: LlmProvider[];
  saving: boolean;
  flushPendingSettingsWrites: () => Promise<void>;
  onChange: (
    update:
      | Partial<TtsLlmPreprocessingSettings>
      | ((
          current: TtsLlmPreprocessingSettings,
        ) => Partial<TtsLlmPreprocessingSettings>),
    field: string,
  ) => Promise<void>;
};

const errorMessage = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

const newPromptId = () =>
  `tts_llm_${Date.now()}_${Math.random().toString(36).slice(2, 9)}`;

const LLM_PROVIDER_DOCUMENTATION: Record<string, string> = {
  openai: "https://developers.openai.com/api/docs/guides/text-generation",
  openrouter: "https://openrouter.ai/docs/quickstart",
  anthropic: "https://docs.anthropic.com/en/docs/intro-to-claude",
  groq: "https://console.groq.com/docs/overview",
  cerebras: "https://inference-docs.cerebras.ai/introduction",
  zai: "https://docs.z.ai/",
  bedrock_mantle:
    "https://docs.aws.amazon.com/bedrock/latest/userguide/what-is-bedrock.html",
};

const AIVORELAY_TTS_GUIDE_URL =
  "https://github.com/MaxITService/AIVORelay/blob/main/CLI-TEXT-TO-SPEECH.md";

const ActionTooltip: React.FC<{
  content?: string | null;
  children: React.ReactNode;
}> = ({ content, children }) =>
  content ? <Tooltip content={content}>{children}</Tooltip> : children;

export const TtsAiCleanup: React.FC<TtsAiCleanupProps> = ({
  mode,
  value,
  providers,
  saving,
  flushPendingSettingsWrites,
  onChange,
}) => {
  const { t } = useTranslation();
  const scope = mode === "interactive" ? "interactive" : "file";
  const enabledField =
    scope === "interactive" ? "interactive_enabled" : "file_enabled";
  const promptsField =
    scope === "interactive" ? "interactive_prompts" : "file_prompts";
  const selectedField =
    scope === "interactive"
      ? "interactive_selected_prompt_id"
      : "file_selected_prompt_id";
  const benchmarkTextField =
    scope === "interactive"
      ? "interactive_benchmark_text"
      : "file_benchmark_text";
  const benchmarkLogField =
    scope === "interactive"
      ? "interactive_benchmark_log"
      : "file_benchmark_log";
  const enabled = value[enabledField];
  const prompts = value[promptsField];
  const selectedId = value[selectedField];
  const selectedPrompt =
    prompts.find((prompt) => prompt.id === selectedId) ?? prompts[0] ?? null;
  const benchmarkText = value[benchmarkTextField];
  const benchmarkLog = value[benchmarkLogField];

  const [error, setError] = useState<string | null>(null);
  const [models, setModels] = useState<string[]>([]);
  const [modelsBusy, setModelsBusy] = useState(false);
  const [benchmarkBusy, setBenchmarkBusy] = useState(false);
  const [hasSeparateKey, setHasSeparateKey] = useState(false);
  const [hasSharedKey, setHasSharedKey] = useState(false);
  const [keyBusy, setKeyBusy] = useState(false);
  const [keyDraft, setKeyDraft] = useState("");
  const [editingKey, setEditingKey] = useState(false);
  const [confirmDeletePrompt, setConfirmDeletePrompt] = useState(false);
  const valueRef = useRef(value);
  const keyStatusGenerationRef = useRef(0);
  const modelsGenerationRef = useRef(0);
  valueRef.current = value;

  const providerOptions = useMemo<SelectOption[]>(
    () =>
      providers
        .filter((provider) => provider.id !== "apple_intelligence")
        .map((provider) => ({ value: provider.id, label: provider.label })),
    [providers],
  );
  const promptOptions = useMemo<SelectOption[]>(
    () =>
      prompts.map((prompt) => ({
        value: prompt.id,
        label: prompt.name,
      })),
    [prompts],
  );
  const modelOptions = useMemo(
    () =>
      Array.from(new Set([value.model, ...models].filter(Boolean)))
        .sort()
        .map((model) => ({ value: model, label: model })),
    [models, value.model],
  );
  const effectiveKeyAvailable =
    value.provider_id === "custom" ||
    (value.key_source === "shared" ? hasSharedKey : hasSeparateKey);
  const providerLabel =
    providerOptions.find((provider) => provider.value === value.provider_id)
      ?.label ?? value.provider_id;
  const providerDocumentationUrl =
    LLM_PROVIDER_DOCUMENTATION[value.provider_id];

  const patch = useCallback(
    (partial: Partial<TtsLlmPreprocessingSettings>, field: string) =>
      onChange(partial, field),
    [onChange],
  );

  const refreshKeyStatus = useCallback(async () => {
    if (!value.provider_id) return;
    const generation = ++keyStatusGenerationRef.current;
    const providerId = value.provider_id;
    setKeyBusy(true);
    try {
      const [separate, shared] = await Promise.all([
        commands.ttsLlmHasApiKey(providerId),
        commands.llmHasStoredApiKey("post_processing", providerId),
      ]);
      if (separate.status === "error") throw new Error(separate.error);
      if (shared.status === "error") throw new Error(shared.error);
      if (
        generation !== keyStatusGenerationRef.current ||
        valueRef.current.provider_id !== providerId
      ) {
        return;
      }
      setHasSeparateKey(separate.data);
      setHasSharedKey(shared.data);
    } catch (caught) {
      if (generation !== keyStatusGenerationRef.current) return;
      setError(errorMessage(caught));
    } finally {
      if (generation === keyStatusGenerationRef.current) {
        setKeyBusy(false);
      }
    }
  }, [value.provider_id]);

  useEffect(() => {
    void refreshKeyStatus();
  }, [refreshKeyStatus]);

  const loadModels = async () => {
    const generation = ++modelsGenerationRef.current;
    const providerId = value.provider_id;
    setModelsBusy(true);
    setError(null);
    try {
      await flushPendingSettingsWrites();
      if (
        generation !== modelsGenerationRef.current ||
        valueRef.current.provider_id !== providerId
      ) {
        return;
      }
      const result = await commands.fetchTtsLlmModels();
      if (result.status === "error") throw new Error(result.error);
      if (
        generation !== modelsGenerationRef.current ||
        valueRef.current.provider_id !== providerId
      ) {
        return;
      }
      setModels(result.data);
    } catch (caught) {
      if (generation !== modelsGenerationRef.current) return;
      setError(errorMessage(caught));
    } finally {
      if (generation === modelsGenerationRef.current) {
        setModelsBusy(false);
      }
    }
  };

  const saveKey = async () => {
    const providerId = value.provider_id;
    setKeyBusy(true);
    setError(null);
    try {
      const result = await commands.ttsLlmSetApiKey(providerId, keyDraft);
      if (result.status === "error") throw new Error(result.error);
      if (valueRef.current.provider_id !== providerId) return;
      setKeyDraft("");
      setEditingKey(false);
      await refreshKeyStatus();
    } catch (caught) {
      if (valueRef.current.provider_id !== providerId) return;
      setError(errorMessage(caught));
    } finally {
      if (valueRef.current.provider_id === providerId) {
        setKeyBusy(false);
      }
    }
  };

  const clearKey = async () => {
    const providerId = value.provider_id;
    setKeyBusy(true);
    setError(null);
    try {
      const result = await commands.ttsLlmClearApiKey(providerId);
      if (result.status === "error") throw new Error(result.error);
      if (valueRef.current.provider_id !== providerId) return;
      setHasSeparateKey(false);
      setEditingKey(true);
    } catch (caught) {
      if (valueRef.current.provider_id !== providerId) return;
      setError(errorMessage(caught));
    } finally {
      if (valueRef.current.provider_id === providerId) {
        setKeyBusy(false);
      }
    }
  };

  const updateSelectedPrompt = (
    partial: Partial<TtsLlmPrompt>,
    field: string,
  ) => {
    if (!selectedPrompt) return Promise.resolve();
    return patch(
      {
        [promptsField]: prompts.map((prompt) =>
          prompt.id === selectedPrompt.id ? { ...prompt, ...partial } : prompt,
        ),
      },
      field,
    );
  };

  const addPrompt = () => {
    const id = newPromptId();
    const baseName = t(
      "textToSpeech.aiCleanup.newPromptName",
      "New cleanup prompt",
    );
    const usedNames = new Set(
      prompts.map((prompt) => prompt.name.trim().toLocaleLowerCase()),
    );
    let name = baseName;
    for (let suffix = 2; usedNames.has(name.toLocaleLowerCase()); suffix += 1) {
      name = `${baseName} ${suffix}`;
    }
    const nextPrompt: TtsLlmPrompt = {
      id,
      name,
      prompt: t(
        "textToSpeech.aiCleanup.newPromptInstructions",
        "Prepare the supplied text for natural speech. Preserve its meaning and return only the cleaned text.",
      ),
    };
    return patch(
      {
        [promptsField]: [...prompts, nextPrompt],
        [selectedField]: id,
      },
      `llm_preprocessing.${promptsField}.collection`,
    );
  };

  const deletePrompt = () => {
    if (!selectedPrompt || prompts.length <= 1) return Promise.resolve();
    const nextPrompts = prompts.filter(
      (prompt) => prompt.id !== selectedPrompt.id,
    );
    return patch(
      {
        [promptsField]: nextPrompts,
        [selectedField]: nextPrompts[0]?.id ?? "",
      },
      `llm_preprocessing.${promptsField}.collection`,
    );
  };

  const runBenchmark = async () => {
    setBenchmarkBusy(true);
    setError(null);
    try {
      await flushPendingSettingsWrites();
      const response = await commands.runTtsLlmBenchmark(scope);
      if (response.status === "error") throw new Error(response.error);
      const result = response.data;
      await onChange(
        (current) => ({
          [benchmarkLogField]: [result, ...current[benchmarkLogField]].slice(
            0,
            100,
          ),
        }),
        `llm_preprocessing.${benchmarkLogField}`,
      );
      if (!result.success) {
        setError(result.error || "TTS AI cleanup benchmark failed");
      }
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBenchmarkBusy(false);
    }
  };

  return (
    <SettingsGroup
      title={t("textToSpeech.aiCleanup.title", "AI text cleanup")}
      description={t(
        "textToSpeech.aiCleanup.description",
        "Optionally rewrite text with an LLM before deterministic rules and speech chunking.",
      )}
      collapsible
      collapsed={value.section_collapsed}
      onCollapsedChange={(collapsed) =>
        void patch(
          { section_collapsed: collapsed },
          "llm_preprocessing.section_collapsed",
        )
      }
      collapseLabel={t("common.collapse", "Collapse")}
      expandLabel={t("common.expand", "Expand")}
      help={
        <TtsHelpDisclosure
          summary={t("textToSpeech.help.aiCleanupSummary", {
            provider: providerLabel,
          })}
          items={[
            {
              term: t("textToSpeech.help.provider"),
              description: t("textToSpeech.help.aiProviderDescription"),
            },
            {
              term: t("textToSpeech.help.model"),
              description: t("textToSpeech.help.aiModelDescription"),
            },
            {
              term: t("textToSpeech.help.prompt"),
              description: t("textToSpeech.help.aiPromptDescription"),
            },
            {
              term: t("textToSpeech.help.privacyAndCost"),
              description: t("textToSpeech.help.aiPrivacyDescription"),
            },
          ]}
          links={[
            ...(providerDocumentationUrl
              ? [
                  {
                    label: t("textToSpeech.help.providerDocumentation"),
                    href: providerDocumentationUrl,
                  },
                ]
              : []),
            {
              label: t("textToSpeech.help.aivoRelayGuide"),
              href: AIVORELAY_TTS_GUIDE_URL,
            },
          ]}
        />
      }
    >
      <ToggleSwitch
        grouped
        checked={enabled}
        onChange={(checked) =>
          void patch(
            { [enabledField]: checked },
            `llm_preprocessing.${enabledField}`,
          )
        }
        isUpdating={saving}
        label={t(
          "textToSpeech.aiCleanup.enable",
          scope === "interactive"
            ? "Clean selected text with AI"
            : "Clean file text with AI",
        )}
        description={t(
          "textToSpeech.aiCleanup.enableDescription",
          "The text is sent to the configured LLM before speech synthesis. This can use paid credits and may send document content to a third party.",
        )}
        descriptionMode="inline"
      />

      <div className="mx-6 my-4 rounded-lg border border-amber-400/25 bg-amber-400/10 px-4 py-3 text-xs leading-relaxed text-amber-100">
        {t(
          "textToSpeech.aiCleanup.privacyWarning",
          "Privacy and cost: when enabled, source text is sent to the selected provider. Review confidential material and provider pricing before use.",
        )}
      </div>

      <SettingContainer
        grouped
        title={t("textToSpeech.aiCleanup.provider", "Provider")}
        description={t(
          "textToSpeech.aiCleanup.providerDescription",
          "Provider definitions are shared with LLM Post Processing; this selection and model are TTS-only.",
        )}
      >
        <Select
          className="min-w-64"
          options={providerOptions}
          value={value.provider_id}
          isClearable={false}
          onChange={(providerId) => {
            if (!providerId) return;
            keyStatusGenerationRef.current += 1;
            modelsGenerationRef.current += 1;
            setKeyBusy(true);
            setModelsBusy(false);
            setModels([]);
            void patch(
              { provider_id: providerId, model: "" },
              "llm_preprocessing.provider_id",
            );
          }}
        />
      </SettingContainer>

      <SettingContainer
        grouped
        title={t("textToSpeech.aiCleanup.keySource", "API key source")}
        description={t(
          "textToSpeech.aiCleanup.keySourceDescription",
          "Reuse the secure key for this provider from LLM Post Processing, or store a separate TTS cleanup key.",
        )}
      >
        <Select
          className="min-w-64"
          options={[
            {
              value: "shared",
              label: t(
                "textToSpeech.aiCleanup.sharedKey",
                "Same as LLM Post Processing",
              ),
            },
            {
              value: "separate",
              label: t(
                "textToSpeech.aiCleanup.separateKey",
                "Separate TTS cleanup key",
              ),
            },
          ]}
          value={value.key_source}
          isClearable={false}
          onChange={(keySource) => {
            if (keySource) {
              void patch(
                { key_source: keySource as "shared" | "separate" },
                "llm_preprocessing.key_source",
              );
            }
          }}
        />
      </SettingContainer>

      {value.key_source === "shared" ? (
        <SettingContainer
          grouped
          title={t("textToSpeech.aiCleanup.sharedKeyStatus", "Shared key")}
          description={t(
            "textToSpeech.aiCleanup.sharedKeyStatusDescription",
            "Manage this credential on the LLM Post Processing page.",
          )}
        >
          <span
            className={`text-sm ${
              effectiveKeyAvailable ? "text-emerald-300" : "text-amber-300"
            }`}
          >
            {keyBusy
              ? t("common.loading", "Loading…")
              : effectiveKeyAvailable
                ? t("textToSpeech.aiCleanup.keyAvailable", "Key available")
                : t(
                    "textToSpeech.aiCleanup.keyMissing",
                    "No shared key for this provider",
                  )}
          </span>
        </SettingContainer>
      ) : (
        <SettingContainer
          grouped
          layout="stacked"
          title={t(
            "textToSpeech.aiCleanup.separateKeyTitle",
            "Separate TTS cleanup API key",
          )}
          description={t(
            "textToSpeech.aiCleanup.separateKeyDescription",
            "Stored securely in Windows Credential Manager and never written to settings JSON.",
          )}
        >
          {hasSeparateKey && !editingKey ? (
            <StoredApiKeyDisplay
              loading={keyBusy}
              onReplace={() => setEditingKey(true)}
              onDelete={() => void clearKey()}
            />
          ) : (
            <ApiKeyEditor
              value={keyDraft}
              loading={keyBusy}
              onChange={setKeyDraft}
              onSave={() => void saveKey()}
              onCancel={() => {
                setEditingKey(false);
                setKeyDraft("");
              }}
              showCancel={hasSeparateKey}
              placeholder={t(
                "textToSpeech.aiCleanup.apiKeyPlaceholder",
                "Paste API key",
              )}
            />
          )}
        </SettingContainer>
      )}

      {value.provider_id === "custom" && (
        <>
          <SettingContainer
            grouped
            title={t("textToSpeech.aiCleanup.baseUrl", "Custom base URL")}
            description={t(
              "textToSpeech.aiCleanup.baseUrlDescription",
              "OpenAI-compatible API root ending at /v1.",
            )}
          >
            <Input
              className="min-w-80"
              value={value.custom_base_url}
              placeholder="https://example.com/v1"
              onChange={(event) =>
                void patch(
                  { custom_base_url: event.target.value },
                  "llm_preprocessing.custom_base_url",
                )
              }
              onBlur={() => void flushPendingSettingsWrites()}
            />
          </SettingContainer>
          <ToggleSwitch
            grouped
            checked={value.custom_allow_insecure_http}
            onChange={(checked) =>
              void patch(
                { custom_allow_insecure_http: checked },
                "llm_preprocessing.custom_allow_insecure_http",
              )
            }
            label={t("textToSpeech.aiCleanup.allowHttp", "Allow insecure HTTP")}
            description={t(
              "textToSpeech.aiCleanup.allowHttpDescription",
              "DANGER: Plain HTTP provides no encryption. If an API key is configured for this endpoint, it and the entire HTTP exchange—including all request and response contents—are exposed in transit. Anyone who can monitor the network can steal the key, read all transmitted data, and modify requests or responses. Never use plain HTTP over the Internet. Enable it only for a local endpoint on a network you fully control.",
            )}
            descriptionMode="inline"
          />
        </>
      )}

      <SettingContainer
        grouped
        layout="stacked"
        title={t("textToSpeech.aiCleanup.model", "Model")}
        description={t(
          "textToSpeech.aiCleanup.modelDescription",
          "Choose a discovered model or enter an exact provider model ID.",
        )}
      >
        <div className="flex flex-wrap items-center gap-2">
          <ModelSelect
            value={value.model}
            options={modelOptions}
            isLoading={modelsBusy}
            placeholder={t(
              "textToSpeech.aiCleanup.modelPlaceholder",
              "Select or enter model ID",
            )}
            onSelect={(model) =>
              void patch({ model }, "llm_preprocessing.model")
            }
            onCreate={(model) =>
              void patch({ model }, "llm_preprocessing.model")
            }
            onBlur={() => void flushPendingSettingsWrites()}
            className="min-w-[300px] flex-1"
          />
          <ActionTooltip
            content={
              modelsBusy
                ? t("textToSpeech.aiCleanup.modelsLoading")
                : keyBusy
                  ? t("textToSpeech.aiCleanup.keyStatusLoading")
                  : !effectiveKeyAvailable
                    ? t("textToSpeech.aiCleanup.apiKeyRequired")
                    : null
            }
          >
            <Button
              variant="secondary"
              size="sm"
              onClick={() => void loadModels()}
              disabled={modelsBusy || keyBusy || !effectiveKeyAvailable}
              className="inline-flex items-center gap-2"
            >
              {modelsBusy ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <RefreshCcw className="h-4 w-4" />
              )}
              {modelsBusy
                ? t("textToSpeech.aiCleanup.loadingModels")
                : t("textToSpeech.aiCleanup.loadModels", "Load models")}
            </Button>
          </ActionTooltip>
        </div>
      </SettingContainer>

      <SettingContainer
        grouped
        layout="stacked"
        title={t("textToSpeech.aiCleanup.prompt", "Cleanup prompt")}
        description={t(
          "textToSpeech.aiCleanup.promptDescription",
          "Prompt collections are independent for interactive reading and File Operations.",
        )}
      >
        <div className="space-y-3">
          <div className="flex flex-wrap gap-2">
            <Select
              className="min-w-64 flex-1"
              options={promptOptions}
              value={selectedPrompt?.id ?? null}
              isClearable={false}
              onChange={(promptId) => {
                if (promptId) {
                  void patch(
                    { [selectedField]: promptId },
                    `llm_preprocessing.${selectedField}`,
                  );
                }
              }}
            />
            <Button
              variant="secondary"
              size="sm"
              onClick={() => void addPrompt()}
              className="inline-flex items-center gap-2"
            >
              <Plus className="h-4 w-4" />
              {t("common.add", "Add")}
            </Button>
            <ActionTooltip
              content={
                prompts.length <= 1
                  ? t("textToSpeech.aiCleanup.keepOnePrompt")
                  : null
              }
            >
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setConfirmDeletePrompt(true)}
                disabled={prompts.length <= 1}
                className="inline-flex items-center gap-2"
              >
                <Trash2 className="h-4 w-4" />
                {t("common.delete", "Delete")}
              </Button>
            </ActionTooltip>
          </div>
          {selectedPrompt && (
            <>
              <Input
                value={selectedPrompt.name}
                maxLength={256}
                placeholder={t(
                  "textToSpeech.aiCleanup.promptName",
                  "Prompt name",
                )}
                onChange={(event) =>
                  void updateSelectedPrompt(
                    { name: event.target.value },
                    `llm_preprocessing.${promptsField}`,
                  )
                }
                onBlur={() => void flushPendingSettingsWrites()}
              />
              <Textarea
                value={selectedPrompt.prompt}
                maxLength={32768}
                className="min-h-40 w-full"
                placeholder={t(
                  "textToSpeech.aiCleanup.promptInstructions",
                  "Instructions sent as the system prompt",
                )}
                onChange={(event) =>
                  void updateSelectedPrompt(
                    { prompt: event.target.value },
                    `llm_preprocessing.${promptsField}`,
                  )
                }
                onBlur={() => void flushPendingSettingsWrites()}
              />
            </>
          )}
        </div>
      </SettingContainer>

      <SettingContainer
        grouped
        title={t("textToSpeech.aiCleanup.chunkSize", "LLM chunk size")}
        description={t(
          "textToSpeech.aiCleanup.chunkSizeDescription",
          "Large documents are split at paragraph or sentence boundaries before cleanup.",
        )}
      >
        <CommittedNumberInput
          className="w-32"
          min={1000}
          max={50000}
          step={500}
          value={value.chunk_target_chars}
          onCommit={(chunk_target_chars) =>
            void patch(
              { chunk_target_chars },
              "llm_preprocessing.chunk_target_chars",
            )
          }
        />
      </SettingContainer>

      <div className="grid gap-0 md:grid-cols-3">
        {[
          {
            key: "retry_count" as const,
            label: t("textToSpeech.aiCleanup.retries", "Retries"),
            description: t("textToSpeech.aiCleanup.retriesDescription"),
            min: 0,
            max: 10,
            step: 1,
          },
          {
            key: "retry_base_delay_ms" as const,
            label: t("textToSpeech.aiCleanup.retryDelay", "Retry delay (ms)"),
            description: t("textToSpeech.aiCleanup.retryDelayDescription"),
            min: 100,
            max: 30000,
            step: 100,
          },
          {
            key: "request_timeout_seconds" as const,
            label: t("textToSpeech.aiCleanup.timeout", "Request timeout (s)"),
            description: t("textToSpeech.aiCleanup.timeoutDescription"),
            min: 10,
            max: 600,
            step: 10,
          },
        ].map((item) => (
          <SettingContainer
            key={item.key}
            grouped
            compact
            title={item.label}
            description={item.description}
          >
            <CommittedNumberInput
              className="w-28"
              min={item.min}
              max={item.max}
              step={item.step}
              value={value[item.key]}
              onCommit={(nextValue) =>
                void patch(
                  { [item.key]: nextValue },
                  `llm_preprocessing.${item.key}`,
                )
              }
            />
          </SettingContainer>
        ))}
      </div>

      <SettingsGroup
        title={t("textToSpeech.aiCleanup.benchmarkTitle", "Test and benchmark")}
        description={t(
          "textToSpeech.aiCleanup.benchmarkDescription",
          "Send synthetic or non-sensitive sample text through the selected prompt and save latency and output for comparison.",
        )}
        collapsible
        collapsed={value.benchmark_collapsed}
        onCollapsedChange={(collapsed) =>
          void patch(
            { benchmark_collapsed: collapsed },
            "llm_preprocessing.benchmark_collapsed",
          )
        }
        help={
          <TtsHelpDisclosure
            summary={t("textToSpeech.help.benchmarkSummary")}
            items={[
              {
                term: t("textToSpeech.help.testText"),
                description: t("textToSpeech.help.testTextDescription"),
              },
              {
                term: t("textToSpeech.help.results"),
                description: t("textToSpeech.help.resultsDescription"),
              },
            ]}
            links={
              providerDocumentationUrl
                ? [
                    {
                      label: t("textToSpeech.help.providerDocumentation"),
                      href: providerDocumentationUrl,
                    },
                  ]
                : [
                    {
                      label: t("textToSpeech.help.aivoRelayGuide"),
                      href: AIVORELAY_TTS_GUIDE_URL,
                    },
                  ]
            }
          />
        }
      >
        <SettingContainer
          grouped
          layout="stacked"
          title={t("textToSpeech.aiCleanup.testText", "Test text")}
          description={t(
            "textToSpeech.aiCleanup.testTextDescription",
            "This text is sent to the configured provider.",
          )}
        >
          <Textarea
            value={benchmarkText}
            className="min-h-32 w-full"
            maxLength={50000}
            onChange={(event) =>
              void patch(
                { [benchmarkTextField]: event.target.value },
                `llm_preprocessing.${benchmarkTextField}`,
              )
            }
            onBlur={() => void flushPendingSettingsWrites()}
          />
          <div className="mt-3 flex justify-end gap-2">
            <ActionTooltip
              content={
                benchmarkBusy
                  ? t("textToSpeech.aiCleanup.benchmarkInProgress")
                  : !benchmarkText.trim()
                    ? t("textToSpeech.aiCleanup.testTextRequired")
                    : !value.model.trim()
                      ? t("textToSpeech.aiCleanup.modelRequired")
                      : !effectiveKeyAvailable
                        ? t("textToSpeech.aiCleanup.apiKeyRequired")
                        : !selectedPrompt?.prompt.trim()
                          ? t("textToSpeech.aiCleanup.promptRequired")
                          : null
              }
            >
              <Button
                variant="primary"
                size="sm"
                onClick={() => void runBenchmark()}
                disabled={
                  benchmarkBusy ||
                  !benchmarkText.trim() ||
                  !value.model.trim() ||
                  !effectiveKeyAvailable ||
                  !selectedPrompt?.prompt.trim()
                }
                className="inline-flex items-center gap-2"
              >
                {benchmarkBusy ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <RefreshCcw className="h-4 w-4" />
                )}
                {benchmarkBusy
                  ? t("textToSpeech.aiCleanup.running", "Running…")
                  : t(
                      "textToSpeech.aiCleanup.runBenchmark",
                      "Run test and benchmark",
                    )}
              </Button>
            </ActionTooltip>
            <ActionTooltip
              content={
                benchmarkLog.length === 0
                  ? t("textToSpeech.aiCleanup.noResultsToClear")
                  : null
              }
            >
              <Button
                variant="secondary"
                size="sm"
                disabled={benchmarkLog.length === 0}
                onClick={() =>
                  void patch(
                    { [benchmarkLogField]: [] },
                    `llm_preprocessing.${benchmarkLogField}`,
                  )
                }
              >
                {t("textToSpeech.aiCleanup.clearResults")}
              </Button>
            </ActionTooltip>
          </div>
        </SettingContainer>

        <SettingContainer
          grouped
          layout="stacked"
          title={t("textToSpeech.aiCleanup.results", "Results")}
          description={t(
            "textToSpeech.aiCleanup.resultsDescription",
            "Up to 100 newest results are retained within a safe settings-storage limit for this TTS scope.",
          )}
        >
          {benchmarkLog.length === 0 ? (
            <p className="text-sm text-[#808080]">
              {t("textToSpeech.aiCleanup.noResults", "No test runs yet.")}
            </p>
          ) : (
            <div className="space-y-2">
              {benchmarkLog.map((result, index) => (
                <details
                  key={`${result.timestamp_ms}-${index}`}
                  className="rounded-md border border-white/[0.06] bg-[#101010]/60 px-3 py-2"
                >
                  <summary className="cursor-pointer text-sm text-[#d8d8d8]">
                    <span
                      className={
                        result.success ? "text-emerald-300" : "text-red-300"
                      }
                    >
                      {result.success ? "✓" : "✕"}
                    </span>{" "}
                    {t("textToSpeech.aiCleanup.benchmarkResult", {
                      provider: result.provider_label,
                      model:
                        result.model || t("textToSpeech.aiCleanup.modelNotSet"),
                      duration: result.duration_ms,
                      input: result.input_chars,
                      output: result.output_chars,
                    })}
                  </summary>
                  <div className="mt-3 space-y-2 text-xs">
                    {result.response_text && (
                      <pre className="max-h-56 overflow-auto whitespace-pre-wrap rounded bg-black/25 p-3 text-[#d8d8d8]">
                        {result.response_text}
                      </pre>
                    )}
                    {result.error && (
                      <pre className="max-h-40 overflow-auto whitespace-pre-wrap rounded bg-red-500/10 p-3 text-red-200">
                        {result.error}
                      </pre>
                    )}
                  </div>
                </details>
              ))}
            </div>
          )}
        </SettingContainer>
      </SettingsGroup>

      {error && (
        <div className="mx-6 my-4 rounded-lg border border-red-500/25 bg-red-500/10 px-4 py-3 text-sm text-red-200">
          {error}
        </div>
      )}
      <ConfirmationModal
        isOpen={confirmDeletePrompt}
        onClose={() => setConfirmDeletePrompt(false)}
        onConfirm={() => void deletePrompt()}
        title={t("textToSpeech.aiCleanup.deletePromptConfirmTitle")}
        message={t("textToSpeech.aiCleanup.deletePromptConfirmMessage", {
          name: selectedPrompt?.name ?? "",
        })}
        confirmText={t("common.delete", "Delete")}
        cancelText={t("common.cancel")}
        variant="danger"
      />
    </SettingsGroup>
  );
};
