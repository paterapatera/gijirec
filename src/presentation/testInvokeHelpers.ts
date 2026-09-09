import {
  DEFAULT_TRANSCRIBE_SETTINGS,
  type GetTranscribeSettingsResponse,
  type LocalAvailability,
} from "../infrastructure/tauri/transcribeSettingsCommands";
import type { TranscribePhaseChanged } from "./hooks/transcribe-status";

const DEFAULT_MOCK_TRANSCRIBE_SETTINGS_RESPONSE: GetTranscribeSettingsResponse = {
  settings: DEFAULT_TRANSCRIBE_SETTINGS,
  local_availability: { q5_0: false, q8_0: false, fp16: true },
};

const DEFAULT_MOCK_TRANSCRIBE_STATUS_RESPONSE = {
  phase: { phase: "ready", timestamp_ms: 1 } satisfies TranscribePhaseChanged,
  model_progress: null,
};

/** Shared invoke handlers for App wiring tests (ModelVariantSelector + useTranscribeStatus). */
export function handleCommonTranscribeInvokeCommands(cmd: string): unknown {
  switch (cmd) {
    case "get_transcribe_settings":
      return DEFAULT_MOCK_TRANSCRIBE_SETTINGS_RESPONSE;
    case "get_transcribe_status":
      return DEFAULT_MOCK_TRANSCRIBE_STATUS_RESPONSE;
    case "get_transcribe_phase":
      return DEFAULT_MOCK_TRANSCRIBE_STATUS_RESPONSE.phase;
    default:
      return undefined;
  }
}

export function coerceLocalAvailability(
  availability: LocalAvailability | undefined,
): LocalAvailability {
  if (
    availability !== undefined &&
    typeof availability.q5_0 === "boolean" &&
    typeof availability.q8_0 === "boolean" &&
    typeof availability.fp16 === "boolean"
  ) {
    return availability;
  }
  return DEFAULT_MOCK_TRANSCRIBE_SETTINGS_RESPONSE.local_availability;
}
