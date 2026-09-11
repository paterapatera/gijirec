import type {
  CaptureAudioControls,
  CaptureAudioControlsState,
} from "../../presentation/hooks/capture-audio-controls-types";
import { defaultInvoke, type InjectableInvokeFn } from "./injectableInvoke";

export interface CaptureAudioControlsCommandsOptions {
  invokeFn?: InjectableInvokeFn;
}

export async function getCaptureAudioControls(
  options: CaptureAudioControlsCommandsOptions = {},
): Promise<CaptureAudioControlsState> {
  const { invokeFn = defaultInvoke } = options;
  return invokeFn<CaptureAudioControlsState>("get_capture_audio_controls");
}

export async function setCaptureAudioControls(
  patch: Partial<CaptureAudioControls>,
  options: CaptureAudioControlsCommandsOptions = {},
): Promise<CaptureAudioControlsState> {
  const { invokeFn = defaultInvoke } = options;
  // Keys stay snake_case; host commands use `rename_all = "snake_case"`.
  return invokeFn<CaptureAudioControlsState>("set_capture_audio_controls", { ...patch });
}
