export type VoiceCommandModelSettings = {
  post_process_models?: Partial<Record<string, string>>;
  voice_command_models?: Partial<Record<string, string>>;
};

export function resolveVoiceCommandModel(
  settings: VoiceCommandModelSettings | null | undefined,
  providerId: string,
  useSameAsPostProcess: boolean,
): string {
  const postProcessModel = settings?.post_process_models?.[providerId] ?? "";
  if (useSameAsPostProcess) {
    return postProcessModel;
  }

  const separateModel = settings?.voice_command_models?.[providerId] ?? "";
  return separateModel.trim() ? separateModel : postProcessModel;
}
