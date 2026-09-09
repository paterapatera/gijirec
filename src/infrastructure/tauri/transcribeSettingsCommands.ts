import { invoke } from "@tauri-apps/api/core";

/** whisper-transcribe-settings.md contract mirror. */
export type WhisperModelVariant = "q5_0" | "q8_0" | "fp16";

export interface TranscribeSettings {
  model_variant: WhisperModelVariant;
}

export type LocalAvailability = Record<WhisperModelVariant, boolean>;

export interface GetTranscribeSettingsResponse {
  settings: TranscribeSettings;
  local_availability: LocalAvailability;
}

export interface SetTranscribeModelVariantRequest {
  model_variant: WhisperModelVariant;
}

export interface SetTranscribeModelVariantResponse {
  settings: TranscribeSettings;
}

export type TranscribeSettingsErrorCode = "SETTINGS_PERSIST_FAILED" | "INVALID_MODEL_VARIANT";

export interface TranscribeSettingsUserError {
  code: TranscribeSettingsErrorCode;
  message_ja: string;
  action_ja: string;
}

export const WHISPER_MODEL_VARIANTS: WhisperModelVariant[] = ["q5_0", "q8_0", "fp16"];

export const WHISPER_MODEL_VARIANT_LABELS: Record<WhisperModelVariant, string> = {
  q5_0: "Q5_0",
  q8_0: "Q8_0",
  fp16: "FP16",
};

export const DEFAULT_TRANSCRIBE_SETTINGS: TranscribeSettings = {
  model_variant: "fp16",
};

export interface TranscribeSettingsCommandsOptions {
  invokeFn?: typeof invoke;
}

export async function getTranscribeSettings(
  options: TranscribeSettingsCommandsOptions = {},
): Promise<GetTranscribeSettingsResponse> {
  const { invokeFn = invoke } = options;
  return invokeFn<GetTranscribeSettingsResponse>("get_transcribe_settings");
}

export async function setTranscribeModelVariant(
  request: SetTranscribeModelVariantRequest,
  options: TranscribeSettingsCommandsOptions = {},
): Promise<SetTranscribeModelVariantResponse> {
  const { invokeFn = invoke } = options;
  return invokeFn<SetTranscribeModelVariantResponse>("set_transcribe_model_variant", {
    ...request,
  });
}
