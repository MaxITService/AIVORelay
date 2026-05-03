import React, { useEffect, useState } from "react";
import { Trans, useTranslation } from "react-i18next";
import { ChevronDown, Loader2, RefreshCcw, Trash2 } from "lucide-react";
import { commands, type LlmPostProcessBenchmarkResult } from "@/bindings";

import { SettingsGroup } from "../../ui/SettingsGroup";
import { TellMeMore } from "../../ui/TellMeMore";
import { SettingContainer } from "../../ui/SettingContainer";
import { Button } from "../../ui/Button";
import { ResetButton } from "../../ui/ResetButton";
import { Input } from "../../ui/Input";
import { Dropdown } from "../../ui/Dropdown";
import { Textarea } from "../../ui/Textarea";
import { PostProcessingToggle } from "../PostProcessingToggle";
import { ProviderSelect } from "../PostProcessingSettingsApi/ProviderSelect";
import { BaseUrlField } from "../PostProcessingSettingsApi/BaseUrlField";
import { ApiKeyField } from "../PostProcessingSettingsApi/ApiKeyField";
import { ModelSelect } from "../PostProcessingSettingsApi/ModelSelect";
import { usePostProcessProviderState } from "../PostProcessingSettingsApi/usePostProcessProviderState";
import { useSettings } from "../../../hooks/useSettings";
import { ExtendedThinkingSection } from "../ExtendedThinkingSection";
import { LlmConfigSection } from "../PostProcessingSettingsApi/LlmConfigSection";



const PostProcessingSettingsApiComponent: React.FC = () => {
  const { t } = useTranslation();
  const postProcessState = usePostProcessProviderState();

  return (
    <div className="divide-y divide-mid-gray/10 space-y-6">
      <LlmConfigSection
        title={t("settings.postProcessing.api.transcription.title")}
        description={t("settings.postProcessing.api.transcription.description")}
        state={postProcessState}
        apiKeyFeature="post_processing"
        reasoningSettingPrefix="post_process"
      />
    </div>
  );
};

const formatBenchmarkTimestamp = (timestampMs: number) =>
  new Date(timestampMs).toLocaleString();

const formatBenchmarkDuration = (durationMs: number) =>
  durationMs > 0 ? `${durationMs.toLocaleString()} ms` : "0 ms";

const formatBenchmarkRate = (charsPerSecond: number) =>
  `${charsPerSecond.toFixed(charsPerSecond >= 100 ? 0 : 1)} chars/sec`;

const PostProcessingBenchmarkComponent: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const [isRunning, setIsRunning] = useState(false);
  const [expandedItems, setExpandedItems] = useState<Set<string>>(new Set());

  const collapsed = getSetting("post_process_benchmark_collapsed") ?? true;
  const customSystemPrompt = getSetting("post_process_benchmark_system_prompt") ?? "";
  const userMessage = getSetting("post_process_benchmark_user_message") ?? "";
  const log = getSetting("post_process_benchmark_log") ?? [];
  // When true (default), benchmark uses the currently selected post-processing prompt.
  // When false, user can type a custom system prompt.
  const useSelectedPrompt = getSetting("post_process_benchmark_use_selected_prompt") ?? true;

  // Resolve the active post-processing prompt text for display and benchmark use.
  const prompts = getSetting("post_process_prompts") ?? [];
  const selectedPromptId = getSetting("post_process_selected_prompt_id") ?? "";
  const activePrompt = prompts.find((p) => p.id === selectedPromptId) ?? null;
  const activePromptText = activePrompt?.prompt ?? "";
  const activePromptName = activePrompt?.name ?? null;

  // The prompt actually sent to the LLM when running the benchmark.
  const effectiveSystemPrompt = useSelectedPrompt ? activePromptText : customSystemPrompt;

  const updateCollapsed = (nextCollapsed: boolean) => {
    updateSetting("post_process_benchmark_collapsed", nextCollapsed);
  };

  const toggleItem = (itemKey: string) => {
    setExpandedItems((current) => {
      const next = new Set(current);
      if (next.has(itemKey)) {
        next.delete(itemKey);
      } else {
        next.add(itemKey);
      }
      return next;
    });
  };

  const runBenchmark = async () => {
    if (isRunning) return;

    setIsRunning(true);
    try {
      const result = await commands.runLlmPostProcessBenchmark(
        effectiveSystemPrompt,
        userMessage,
      );
      if (result.status === "ok") {
        const nextLog = [result.data, ...log].slice(0, 100);
        await updateSetting("post_process_benchmark_log", nextLog);
      } else {
        console.error("Failed to run LLM post-processing benchmark:", result.error);
      }
    } catch (error) {
      console.error("Failed to run LLM post-processing benchmark:", error);
    } finally {
      setIsRunning(false);
    }
  };

  const clearLog = async () => {
    setExpandedItems(new Set());
    await updateSetting("post_process_benchmark_log", []);
  };

  const removeLogItem = async (index: number) => {
    const nextLog = [...log];
    nextLog.splice(index, 1);
    await updateSetting("post_process_benchmark_log", nextLog);
  };

  const renderLogDetails = (item: LlmPostProcessBenchmarkResult) => (
    <div className="space-y-3 border-t border-white/[0.05] px-4 py-3 text-xs text-[#d8d8d8]">
      <div className="grid gap-2 sm:grid-cols-2">
        <div>
          <span className="text-[#a0a0a0]">Provider:</span>{" "}
          {item.provider_label || item.provider_id || "Unknown"}
        </div>
        <div>
          <span className="text-[#a0a0a0]">Model:</span>{" "}
          {item.model || "Not configured"}
        </div>
        <div>
          <span className="text-[#a0a0a0]">Duration:</span>{" "}
          {formatBenchmarkDuration(item.duration_ms)}
        </div>
        <div>
          <span className="text-[#a0a0a0]">Input / output:</span>{" "}
          {item.input_chars.toLocaleString()} /{" "}
          {item.output_chars.toLocaleString()} chars
        </div>
      </div>

      <div className="space-y-1.5">
        <div className="font-semibold text-[#f5f5f5]">System Prompt</div>
        <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded-md border border-white/[0.06] bg-[#101010]/80 p-3 text-[#d8d8d8]">
          {item.system_prompt || "(empty)"}
        </pre>
      </div>

      <div className="space-y-1.5">
        <div className="font-semibold text-[#f5f5f5]">User Message</div>
        <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded-md border border-white/[0.06] bg-[#101010]/80 p-3 text-[#d8d8d8]">
          {item.user_message || "(empty)"}
        </pre>
      </div>

      {item.response_text && (
        <div className="space-y-1.5">
          <div className="font-semibold text-[#f5f5f5]">Response</div>
          <pre className="max-h-64 overflow-auto whitespace-pre-wrap rounded-md border border-white/[0.06] bg-[#101010]/80 p-3 text-[#d8d8d8]">
            {item.response_text}
          </pre>
        </div>
      )}

      {item.error && (
        <div className="space-y-1.5">
          <div className="font-semibold text-red-300">Error Details</div>
          <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded-md border border-red-500/20 bg-red-500/10 p-3 text-red-200">
            {item.error}
          </pre>
        </div>
      )}
    </div>
  );

  return (
    <SettingsGroup
      title={t("settings.postProcessing.benchmark.title", "Benchmark")}
      description={t(
        "settings.postProcessing.benchmark.description",
        "Want to see how fast your model processes your own correction example? Add typical recognition mistakes here, run the benchmark, and compare latency and output quality across models.",
      )}
      collapsible={true}
      collapsed={collapsed}
      collapseLabel={t("settings.postProcessing.benchmark.collapse", "Collapse")}
      expandLabel={t("settings.postProcessing.benchmark.expand", "Expand")}
      onCollapsedChange={updateCollapsed}
    >
      <SettingContainer
        title={t(
          "settings.postProcessing.benchmark.systemPrompt.title",
          "System Prompt",
        )}
        description={t(
          "settings.postProcessing.benchmark.systemPrompt.description",
          "The correction instructions sent as the system message for this benchmark.",
        )}
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        {useSelectedPrompt ? (
          // "Use Active Prompt" mode: show read-only info about the active prompt.
          <div className="space-y-2">
            <div className="flex items-start gap-3 rounded-md border border-white/[0.07] bg-[#101010]/60 px-4 py-3">
              <div className="flex-1 min-w-0">
                {activePrompt ? (
                  <>
                    <p className="text-xs text-mid-gray/70">
                      {t(
                        "settings.postProcessing.benchmark.systemPrompt.usingActive",
                        "Using your currently selected post-processing prompt:",
                      )}
                    </p>
                    <p className="mt-0.5 truncate text-sm font-semibold text-[#f5f5f5]">
                      {activePromptName}
                    </p>
                    <pre className="mt-2 max-h-24 overflow-auto whitespace-pre-wrap text-xs text-mid-gray/80 leading-relaxed">
                      {activePromptText}
                    </pre>
                  </>
                ) : (
                  <p className="text-xs text-yellow-400/80">
                    {t(
                      "settings.postProcessing.benchmark.systemPrompt.noActivePrompt",
                      "No post-processing prompt is selected. Select one above or switch to Custom.",
                    )}
                  </p>
                )}
              </div>
            </div>
            <div className="flex justify-end">
              <Button
                onClick={() => updateSetting("post_process_benchmark_use_selected_prompt", false)}
                variant="secondary"
                size="md"
              >
                {t("settings.postProcessing.benchmark.systemPrompt.customize", "Customize")}
              </Button>
            </div>
          </div>
        ) : (
          // "Custom" mode: editable textarea + button to revert to active prompt.
          <div className="space-y-2">
            <Textarea
              value={customSystemPrompt}
              onChange={(event) =>
                updateSetting(
                  "post_process_benchmark_system_prompt",
                  event.target.value,
                )
              }
              disabled={isUpdating("post_process_benchmark_system_prompt")}
              className="w-full"
            />
            <div className="flex justify-end">
              <Button
                onClick={() => updateSetting("post_process_benchmark_use_selected_prompt", true)}
                variant="secondary"
                size="md"
              >
                {t(
                  "settings.postProcessing.benchmark.systemPrompt.useActive",
                  "Use Active Prompt",
                )}
              </Button>
            </div>
          </div>
        )}
      </SettingContainer>

      <SettingContainer
        title={t(
          "settings.postProcessing.benchmark.userMessage.title",
          "User Message",
        )}
        description={t(
          "settings.postProcessing.benchmark.userMessage.description",
          "Add recognition mistakes, bad punctuation, repeated words, casing problems, spoken punctuation, or other examples that match your real dictation.",
        )}
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <Textarea
          value={userMessage}
          onChange={(event) =>
            updateSetting(
              "post_process_benchmark_user_message",
              event.target.value,
            )
          }
          disabled={isUpdating("post_process_benchmark_user_message")}
          className="w-full"
        />
        <p className="mt-2 text-xs text-mid-gray/70">
          Add typical recognition mistakes, bad punctuation, repeated words,
          casing problems, and spoken punctuation to compare models fairly.
        </p>
      </SettingContainer>

      <SettingContainer
        title={t("settings.postProcessing.benchmark.actions.title", "Run")}
        description={t(
          "settings.postProcessing.benchmark.actions.description",
          "Send the current prompts to the configured post-processing provider and save the measured result.",
        )}
        descriptionMode="inline"
        grouped={true}
      >
        <div className="flex flex-wrap justify-end gap-2">
          <Button
            onClick={runBenchmark}
            variant="primary"
            size="md"
            disabled={isRunning || !userMessage.trim()}
            className="inline-flex items-center gap-2"
          >
            {isRunning ? (
              <Loader2 className="h-4 w-4 animate-spin" aria-hidden="true" />
            ) : (
              <RefreshCcw className="h-4 w-4" aria-hidden="true" />
            )}
            {isRunning
              ? t("settings.postProcessing.benchmark.running", "Running...")
              : t("settings.postProcessing.benchmark.run", "Run Benchmark")}
          </Button>
          <Button
            onClick={clearLog}
            variant="secondary"
            size="md"
            disabled={log.length === 0 || isUpdating("post_process_benchmark_log")}
            className="inline-flex items-center gap-2"
          >
            <Trash2 className="h-4 w-4" aria-hidden="true" />
            {t("settings.postProcessing.benchmark.clearLog", "Clear Log")}
          </Button>
        </div>
      </SettingContainer>

      <SettingContainer
        title={t("settings.postProcessing.benchmark.log.title", "Log")}
        description={t(
          "settings.postProcessing.benchmark.log.description",
          "Newest benchmark results are kept first and persisted with settings. Maximum of 100 entries, then they will be overwritten.",
        )}
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        {log.length === 0 ? (
          <div className="rounded-md border border-white/[0.06] bg-[#101010]/50 p-3 text-sm text-mid-gray">
            {t(
              "settings.postProcessing.benchmark.log.empty",
              "No benchmark runs yet.",
            )}
          </div>
        ) : (
          <div className="space-y-2">
            {log.map((item, index) => {
              const itemKey = `${item.timestamp_ms}-${index}`;
              const expanded = expandedItems.has(itemKey);
              const statusClass = item.success ? "text-emerald-300" : "text-red-300";
              return (
                <div
                  key={itemKey}
                  className="overflow-hidden rounded-md border border-white/[0.06] bg-[#101010]/50"
                >
                  <button
                    type="button"
                    onClick={() => toggleItem(itemKey)}
                    className="flex w-full items-center justify-between gap-3 px-4 py-3 text-left transition-colors hover:bg-white/[0.03] focus:outline-none focus:ring-2 focus:ring-[#ff4d8d]/35"
                    aria-expanded={expanded}
                  >
                    <span className="grid min-w-0 flex-1 gap-1 text-xs sm:grid-cols-[1.2fr_1fr_0.8fr_0.8fr_0.7fr] sm:items-center">
                      <span className="truncate font-semibold text-[#f5f5f5]">
                        {item.model || "No model"}
                      </span>
                      <span className="truncate text-[#d8d8d8]">
                        {item.provider_label || item.provider_id || "Unknown"}
                      </span>
                      <span className="text-[#d8d8d8]">
                        {formatBenchmarkDuration(item.duration_ms)}
                      </span>
                      <span className="text-[#d8d8d8]">
                        {formatBenchmarkRate(item.chars_per_second)}
                      </span>
                      <span className={statusClass}>
                        {item.success ? "Success" : "Error"}
                      </span>
                      <span className="text-[#a0a0a0] sm:col-span-5">
                        {formatBenchmarkTimestamp(item.timestamp_ms)}
                      </span>
                    </span>
                    <div className="flex items-center gap-1">
                      <button
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          void removeLogItem(index);
                        }}
                        className="p-1 text-[#a0a0a0] hover:text-red-400 transition-colors"
                        title="Delete entry"
                        aria-label="Delete entry"
                      >
                        <Trash2 className="h-4 w-4" />
                      </button>
                      <ChevronDown
                        className={`h-4 w-4 shrink-0 text-[#a0a0a0] transition-transform ${
                          expanded ? "rotate-180" : "rotate-0"
                        }`}
                        aria-hidden="true"
                      />
                    </div>
                  </button>
                  {expanded && renderLogDetails(item)}
                </div>
              );
            })}
          </div>
        )}
      </SettingContainer>
    </SettingsGroup>
  );
};

const PostProcessingSettingsPromptsComponent: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating, refreshSettings } =
    useSettings();
  const [isCreating, setIsCreating] = useState(false);
  const [draftName, setDraftName] = useState("");
  const [draftText, setDraftText] = useState("");

  const enabled = getSetting("post_process_enabled") || false;
  const prompts = getSetting("post_process_prompts") || [];
  const selectedPromptId = getSetting("post_process_selected_prompt_id") || "";
  const selectedPrompt =
    prompts.find((prompt) => prompt.id === selectedPromptId) || null;

  useEffect(() => {
    if (isCreating) return;

    if (selectedPrompt) {
      setDraftName(selectedPrompt.name);
      setDraftText(selectedPrompt.prompt);
    } else {
      setDraftName("");
      setDraftText("");
    }
  }, [
    isCreating,
    selectedPromptId,
    selectedPrompt?.name,
    selectedPrompt?.prompt,
  ]);

  const handlePromptSelect = (promptId: string | null) => {
    if (!promptId) return;
    updateSetting("post_process_selected_prompt_id", promptId);
    setIsCreating(false);
  };

  const handleCreatePrompt = async () => {
    if (!draftName.trim() || !draftText.trim()) return;

    try {
      const result = await commands.addPostProcessPrompt(
        draftName.trim(),
        draftText.trim(),
      );
      if (result.status === "ok") {
        await refreshSettings();
        updateSetting("post_process_selected_prompt_id", result.data.id);
        setIsCreating(false);
      }
    } catch (error) {
      console.error("Failed to create prompt:", error);
    }
  };

  const handleUpdatePrompt = async () => {
    if (!selectedPromptId || !draftName.trim() || !draftText.trim()) return;

    try {
      await commands.updatePostProcessPrompt(
        selectedPromptId,
        draftName.trim(),
        draftText.trim(),
      );
      await refreshSettings();
    } catch (error) {
      console.error("Failed to update prompt:", error);
    }
  };

  const handleDeletePrompt = async (promptId: string) => {
    if (!promptId) return;

    try {
      await commands.deletePostProcessPrompt(promptId);
      await refreshSettings();
      setIsCreating(false);
    } catch (error) {
      console.error("Failed to delete prompt:", error);
    }
  };

  const handleCancelCreate = () => {
    setIsCreating(false);
    if (selectedPrompt) {
      setDraftName(selectedPrompt.name);
      setDraftText(selectedPrompt.prompt);
    } else {
      setDraftName("");
      setDraftText("");
    }
  };

  const handleStartCreate = () => {
    setIsCreating(true);
    setDraftName("");
    setDraftText("");
  };



  const hasPrompts = prompts.length > 0;
  const isDirty =
    !!selectedPrompt &&
    (draftName.trim() !== selectedPrompt.name ||
      draftText.trim() !== selectedPrompt.prompt.trim());

  return (
    <SettingContainer
      title={t("settings.postProcessing.prompts.selectedPrompt.title")}
      description={t(
        "settings.postProcessing.prompts.selectedPrompt.description",
      )}
      descriptionMode="inline"
      layout="stacked"
      grouped={true}
    >
      <div className="space-y-3">
        <div className="flex gap-2">
          <Dropdown
            selectedValue={selectedPromptId || null}
            options={prompts.map((p) => ({
              value: p.id,
              label: p.name,
            }))}
            onSelect={(value) => handlePromptSelect(value)}
            placeholder={
              prompts.length === 0
                ? t("settings.postProcessing.prompts.noPrompts")
                : t("settings.postProcessing.prompts.selectPrompt")
            }
            disabled={
              isUpdating("post_process_selected_prompt_id") || isCreating
            }
            className="flex-1"
          />
          <Button
            onClick={handleStartCreate}
            variant="primary"
            size="md"
            disabled={isCreating}
          >
            {t("settings.postProcessing.prompts.createNew")}
          </Button>
        </div>

        {!isCreating && hasPrompts && selectedPrompt && (
          <div className="space-y-3">
            <div className="space-y-2 flex flex-col">
              <label className="text-sm font-semibold">
                {t("settings.postProcessing.prompts.promptLabel")}
              </label>
              <Input
                type="text"
                value={draftName}
                onChange={(e) => setDraftName(e.target.value)}
                placeholder={t(
                  "settings.postProcessing.prompts.promptLabelPlaceholder",
                )}
                variant="compact"
              />
            </div>

            <div className="space-y-2 flex flex-col">
              <label className="text-sm font-semibold">
                {t("settings.postProcessing.prompts.promptInstructions")}
              </label>
              <Textarea
                value={draftText}
                onChange={(e) => setDraftText(e.target.value)}
                placeholder={t(
                  "settings.postProcessing.prompts.promptInstructionsPlaceholder",
                )}
              />
              <p className="text-xs text-mid-gray/70">
                <Trans
                  i18nKey="settings.postProcessing.prompts.promptTip"
                  components={{ code: <code /> }}
                />
              </p>
            </div>

            <div className="flex gap-2 pt-2">
              <Button
                onClick={handleUpdatePrompt}
                variant="primary"
                size="md"
                disabled={!draftName.trim() || !draftText.trim() || !isDirty}
              >
                {t("settings.postProcessing.prompts.updatePrompt")}
              </Button>
              <Button
                onClick={() => handleDeletePrompt(selectedPromptId)}
                variant="secondary"
                size="md"
                disabled={!selectedPromptId || prompts.length <= 1}
              >
                {t("settings.postProcessing.prompts.deletePrompt")}
              </Button>
            </div>
          </div>
        )}

        {!isCreating && !selectedPrompt && (
          <div className="p-3 bg-mid-gray/5 rounded border border-mid-gray/20">
            <p className="text-sm text-mid-gray">
              {hasPrompts
                ? t("settings.postProcessing.prompts.selectToEdit")
                : t("settings.postProcessing.prompts.createFirst")}
            </p>
          </div>
        )}

        {isCreating && (
          <div className="space-y-3">
            <div className="space-y-2 block flex flex-col">
              <label className="text-sm font-semibold text-text">
                {t("settings.postProcessing.prompts.promptLabel")}
              </label>
              <Input
                type="text"
                value={draftName}
                onChange={(e) => setDraftName(e.target.value)}
                placeholder={t(
                  "settings.postProcessing.prompts.promptLabelPlaceholder",
                )}
                variant="compact"
              />
            </div>

            <div className="space-y-2 flex flex-col">
              <label className="text-sm font-semibold">
                {t("settings.postProcessing.prompts.promptInstructions")}
              </label>
              <Textarea
                value={draftText}
                onChange={(e) => setDraftText(e.target.value)}
                placeholder={t(
                  "settings.postProcessing.prompts.promptInstructionsPlaceholder",
                )}
              />
              <p className="text-xs text-mid-gray/70">
                <Trans
                  i18nKey="settings.postProcessing.prompts.promptTip"
                  components={{ code: <code /> }}
                />
              </p>
            </div>

            <div className="flex gap-2 pt-2">
              <Button
                onClick={handleCreatePrompt}
                variant="primary"
                size="md"
                disabled={!draftName.trim() || !draftText.trim()}
              >
                {t("settings.postProcessing.prompts.createPrompt")}
              </Button>
              <Button
                onClick={handleCancelCreate}
                variant="secondary"
                size="md"
              >
                {t("settings.postProcessing.prompts.cancel")}
              </Button>
            </div>
          </div>
        )}
      </div>
    </SettingContainer>
  );
};

export const PostProcessingSettingsApi = React.memo(
  PostProcessingSettingsApiComponent,
);
PostProcessingSettingsApi.displayName = "PostProcessingSettingsApi";

export const PostProcessingSettingsPrompts = React.memo(
  PostProcessingSettingsPromptsComponent,
);
PostProcessingSettingsPrompts.displayName = "PostProcessingSettingsPrompts";

export const PostProcessingSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const transcriptionProvider = String(
    getSetting("transcription_provider") || "local",
  );
  const isSonioxProvider = transcriptionProvider === "remote_soniox";
  const isDeepgramProvider = transcriptionProvider === "remote_deepgram";

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      {(isSonioxProvider || isDeepgramProvider) && (
        <div className="p-3 bg-yellow-500/10 border border-yellow-500/30 rounded-lg text-sm text-yellow-400">
          {isDeepgramProvider
            ? t(
                "settings.postProcessing.deepgramWarning",
                "You are using Deepgram live transcription. LLM post-processing is skipped during the standard live cycle.",
              )
            : t("settings.postProcessing.sonioxWarning")}
        </div>
      )}

      {/* Help Section */}
      <TellMeMore title={t("settings.postProcessing.tellMeMore.title")}>
        <div className="space-y-3">
          <p>
            <strong>{t("settings.postProcessing.tellMeMore.headline")}</strong>
          </p>
          <p className="opacity-90">
            {t("settings.postProcessing.tellMeMore.intro")}
          </p>
          <ul className="list-disc list-inside space-y-2 ml-1 opacity-90">
            <li>
              <strong>{t("settings.postProcessing.tellMeMore.apiKey.title")}</strong>{" "}
              {t("settings.postProcessing.tellMeMore.apiKey.description")}
              <p className="ml-5 mt-1 text-xs text-text/70 italic">
                {t("settings.postProcessing.tellMeMore.apiKey.securityNote")}
              </p>
            </li>
            <li>
              <strong>{t("settings.postProcessing.tellMeMore.provider.title")}</strong>{" "}
              {t("settings.postProcessing.tellMeMore.provider.description")}
            </li>
            <li>
              <strong>{t("settings.postProcessing.tellMeMore.model.title")}</strong>{" "}
              {t("settings.postProcessing.tellMeMore.model.description")}
            </li>
            <li>
              <strong>{t("settings.postProcessing.tellMeMore.prompts.title")}</strong>{" "}
              {t("settings.postProcessing.tellMeMore.prompts.description")}
            </li>
          </ul>
          <div className="mt-3 p-2 bg-accent/10 border border-accent/20 rounded-md text-xs">
            <p className="mb-1">{t("settings.postProcessing.tellMeMore.tip")}</p>
            <a
              href="https://openrouter.ai"
              target="_blank"
              rel="noopener noreferrer"
              className="text-accent hover:underline font-medium"
            >
              openrouter.ai
            </a>
          </div>
          <div className="mt-3 p-3 bg-red-500/10 border border-red-500/30 rounded-md">
            <p className="text-sm font-semibold text-red-400 mb-1">
              {t("settings.postProcessing.tellMeMore.privacyWarning.title")}
            </p>
            <p className="text-xs text-red-300/90">
              {t("settings.postProcessing.tellMeMore.privacyWarning.description")}
            </p>
          </div>
        </div>
      </TellMeMore>

      <SettingsGroup title={t("settings.postProcessing.prompts.title")}>
        <PostProcessingToggle descriptionMode="inline" grouped={true} />
        <PostProcessingSettingsPrompts />
      </SettingsGroup>

      <SettingsGroup title={t("settings.postProcessing.api.title")}>
        <PostProcessingSettingsApi />
      </SettingsGroup>

      <PostProcessingBenchmarkComponent />
    </div>
  );
};
