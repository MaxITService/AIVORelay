import React from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, FolderOpen } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { SettingContainer } from "@/components/ui/SettingContainer";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { ToggleSwitch } from "@/components/ui/ToggleSwitch";
import { Tooltip } from "@/components/ui/Tooltip";
import { AIVORELAY_TTS_GUIDE_URL } from "@/lib/tts/ttsProviderMetadata";
import { CommittedNumberInput } from "./CommittedNumberInput";
import { TtsHelpDisclosure } from "./TtsHelpDisclosure";

type TtsFolderAutomationProps = {
  tts: any;
  savingField: string | null;
  updateTts: (patch: any, field: string) => Promise<void>;
};

const ActionTooltip: React.FC<{
  content?: string | null;
  children: React.ReactNode;
}> = ({ content, children }) =>
  content ? <Tooltip content={content}>{children}</Tooltip> : children;

export const TtsFolderAutomation: React.FC<TtsFolderAutomationProps> = ({
  tts,
  savingField,
  updateTts,
}) => {
  const { t } = useTranslation();
  const enabled = Boolean(tts?.watch_folder_enabled ?? false);
  const recursive = Boolean(tts?.watch_recursive ?? false);
  const inputDirectory = String(tts?.watch_input_directory ?? "");
  const outputDirectory = String(tts?.watch_output_directory ?? "");
  const settleDelayMs = Number(tts?.watch_settle_delay_ms ?? 1500);
  const diskReserveMb = Number(tts?.disk_reserve_mb ?? 512);

  const chooseDirectory = async (
    field: "watch_input_directory" | "watch_output_directory",
  ) => {
    const selected = await open({
      directory: true,
      multiple: false,
    });
    if (typeof selected === "string") {
      await updateTts({ [field]: selected }, field);
    }
  };

  return (
    <SettingsGroup
      title={t("textToSpeech.folder.title")}
      description={t("textToSpeech.folder.description")}
      help={
        <TtsHelpDisclosure
          summary={t("textToSpeech.help.folderSummary")}
          items={[
            {
              term: t("textToSpeech.help.inputFolder"),
              description: t("textToSpeech.help.inputFolderDescription"),
            },
            {
              term: t("textToSpeech.help.settleDelay"),
              description: t("textToSpeech.help.settleDelayDescription"),
            },
            {
              term: t("textToSpeech.help.diskReserve"),
              description: t("textToSpeech.help.diskReserveDescription"),
            },
          ]}
          links={[
            {
              label: t("textToSpeech.help.aivoRelayGuide"),
              href: AIVORELAY_TTS_GUIDE_URL,
            },
          ]}
        />
      }
    >
      <div className="px-6 py-4">
        <div
          role="alert"
          className="flex items-start gap-3 rounded-lg border border-amber-400/60 bg-amber-400/15 px-4 py-3 text-amber-100"
        >
          <AlertTriangle
            className="mt-0.5 h-5 w-5 shrink-0 text-amber-300"
            aria-hidden="true"
          />
          <div>
            <p className="text-sm font-bold tracking-wide">
              {t("textToSpeech.folder.caution")}
            </p>
            <p className="mt-1 text-xs leading-relaxed text-amber-100/80">
              {t("textToSpeech.folder.cautionDescription")}
            </p>
          </div>
        </div>
      </div>

      <ToggleSwitch
        grouped
        checked={enabled}
        disabled={!inputDirectory || !outputDirectory}
        onChange={(nextEnabled) =>
          void updateTts(
            { watch_folder_enabled: nextEnabled },
            "watch_folder_enabled",
          )
        }
        isUpdating={savingField === "watch_folder_enabled"}
        label={t("textToSpeech.folder.enable")}
        description={t("textToSpeech.folder.enableDescription")}
        descriptionMode="inline"
      />

      <ToggleSwitch
        grouped
        checked={recursive}
        onChange={(nextRecursive) =>
          void updateTts({ watch_recursive: nextRecursive }, "watch_recursive")
        }
        isUpdating={savingField === "watch_recursive"}
        label={t("textToSpeech.folder.recursive")}
        description={t("textToSpeech.folder.recursiveDescription")}
        descriptionMode="inline"
      />

      <SettingContainer
        grouped
        layout="stacked"
        title={t("textToSpeech.folder.input")}
        description={t("textToSpeech.folder.inputDescription")}
        descriptionMode="inline"
      >
        <div className="flex gap-2">
          <Input
            className="min-w-0 flex-1"
            value={inputDirectory}
            readOnly
            placeholder={t("textToSpeech.folder.noInput")}
          />
          <ActionTooltip
            content={
              savingField !== null
                ? t("textToSpeech.disabledWhileSaving")
                : null
            }
          >
            <Button
              variant="secondary"
              disabled={savingField !== null}
              onClick={() => void chooseDirectory("watch_input_directory")}
            >
              <FolderOpen className="mr-2 inline h-4 w-4" />
              {t("textToSpeech.folder.chooseInput")}
            </Button>
          </ActionTooltip>
        </div>
      </SettingContainer>

      <SettingContainer
        grouped
        layout="stacked"
        title={t("textToSpeech.folder.output")}
        description={t("textToSpeech.folder.outputDescription")}
        descriptionMode="inline"
      >
        <div className="flex gap-2">
          <Input
            className="min-w-0 flex-1"
            value={outputDirectory}
            readOnly
            placeholder={t("textToSpeech.folder.noOutput")}
          />
          <ActionTooltip
            content={
              savingField !== null
                ? t("textToSpeech.disabledWhileSaving")
                : null
            }
          >
            <Button
              variant="secondary"
              disabled={savingField !== null}
              onClick={() => void chooseDirectory("watch_output_directory")}
            >
              <FolderOpen className="mr-2 inline h-4 w-4" />
              {t("textToSpeech.folder.chooseOutput")}
            </Button>
          </ActionTooltip>
        </div>
      </SettingContainer>

      <SettingContainer
        grouped
        title={t("textToSpeech.folder.settleDelay")}
        description={t("textToSpeech.folder.settleDelayDescription")}
        descriptionMode="inline"
        disabled={!enabled}
      >
        <div className="flex items-center gap-2">
          <CommittedNumberInput
            className="w-28"
            min={100}
            max={60000}
            step={100}
            value={settleDelayMs}
            disabled={!enabled}
            onCommit={(watch_settle_delay_ms) =>
              void updateTts(
                { watch_settle_delay_ms },
                "watch_settle_delay_ms",
              )
            }
          />
          <span className="text-xs text-[#808080]">
            {t("textToSpeech.units.milliseconds")}
          </span>
        </div>
      </SettingContainer>

      <SettingContainer
        grouped
        title={t("textToSpeech.folder.diskReserve")}
        description={t("textToSpeech.folder.diskReserveDescription")}
        descriptionMode="inline"
        disabled={!enabled}
      >
        <div className="flex items-center gap-2">
          <CommittedNumberInput
            className="w-28"
            min={0}
            max={1048576}
            step={128}
            value={diskReserveMb}
            disabled={!enabled}
            onCommit={(disk_reserve_mb) =>
              void updateTts(
                { disk_reserve_mb },
                "disk_reserve_mb",
              )
            }
          />
          <span className="text-xs text-[#808080]">
            {t("textToSpeech.units.megabytes")}
          </span>
        </div>
      </SettingContainer>

      <div className="px-6 py-4 text-xs leading-relaxed text-[#a0a0a0]">
        {t("textToSpeech.folder.footer")}
      </div>
      <div className="px-6 py-4 text-xs leading-relaxed text-amber-200/80">
        {t("textToSpeech.folder.eventReliability")}
      </div>
    </SettingsGroup>
  );
};

export default TtsFolderAutomation;
