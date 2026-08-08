import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands } from "@/bindings";
import { useSettings } from "../../hooks/useSettings";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";

interface ChannelSelectorProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const ChannelSelector: React.FC<ChannelSelectorProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating, isLoading } = useSettings();
    const [channelCount, setChannelCount] = useState(1);

    const selectedMicrophone = getSetting("selected_microphone") || "Default";
    const selectedChannel = getSetting("selected_channel");

    useEffect(() => {
      let cancelled = false;
      setChannelCount(1);

      const fetchChannels = async () => {
        try {
          const deviceName =
            selectedMicrophone === "Default"
              ? "default"
              : selectedMicrophone;
          const result = await commands.getMicrophoneChannels(deviceName);
          if (!cancelled && result.status === "ok") {
            setChannelCount(result.data);
          }
        } catch (error) {
          console.error("Failed to get microphone channel count:", error);
        }
      };

      void fetchChannels();
      return () => {
        cancelled = true;
      };
    }, [selectedMicrophone]);

    if (channelCount <= 1) {
      return null;
    }

    const options = [
      { value: "average", label: t("settings.sound.channel.average") },
      ...Array.from({ length: channelCount }, (_, index) => ({
        value: index.toString(),
        label: t("settings.sound.channel.channel", { n: index + 1 }),
      })),
    ];
    const currentValue =
      selectedChannel == null || selectedChannel >= channelCount
        ? "average"
        : selectedChannel.toString();

    return (
      <SettingContainer
        title={t("settings.sound.channel.title")}
        description={t("settings.sound.channel.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <Dropdown
          options={options}
          selectedValue={currentValue}
          onSelect={(value) =>
            updateSetting(
              "selected_channel",
              value === "average" ? null : Number.parseInt(value, 10),
            )
          }
          disabled={isUpdating("selected_channel") || isLoading}
        />
      </SettingContainer>
    );
  },
);

ChannelSelector.displayName = "ChannelSelector";
