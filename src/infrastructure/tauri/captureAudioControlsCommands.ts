import { invoke } from "@tauri-apps/api/core";
import type {
  CaptureAudioControls,
  CaptureAudioControlsState,
} from "../../presentation/hooks/capture-audio-controls-types";

export interface CaptureAudioControlsCommandsOptions {
  invokeFn?: typeof invoke;
}

export async function getCaptureAudioControls(
  options: CaptureAudioControlsCommandsOptions = {},
): Promise<CaptureAudioControlsState> {
  const { invokeFn = invoke } = options;
  return invokeFn<CaptureAudioControlsState>("get_capture_audio_controls");
}

export async function setCaptureAudioControls(
  patch: Partial<CaptureAudioControls>,
  options: CaptureAudioControlsCommandsOptions = {},
): Promise<CaptureAudioControlsState> {
  const { invokeFn = invoke } = options;
  // Keys stay snake_case; host commands use `rename_all = "snake_case"`.
  return invokeFn<CaptureAudioControlsState>("set_capture_audio_controls", { ...patch });
}
