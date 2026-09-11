import type {
  SaveTranscriptSessionRequest,
  SaveTranscriptSessionResult,
} from "../../domain/transcript/saveSession";
import type { EditorSettings } from "../../domain/transcript/types";
import { defaultInvoke, type InjectableInvokeFn } from "./injectableInvoke";

export type { SaveTranscriptSessionRequest, SaveTranscriptSessionResult };

/** transcript-editor-settings.md contract mirror. */
export interface SetEditorSettingsRequest {
  save_directory?: string | null;
  export_jsonl_enabled?: boolean;
}

export type GetEditorSettingsResponse = EditorSettings;
export type SetEditorSettingsResponse = EditorSettings;
export type PickSaveDirectoryResponse = string | null;

export interface EditorCommandsOptions {
  invokeFn?: InjectableInvokeFn;
}

export async function saveTranscriptSession(
  request: SaveTranscriptSessionRequest,
  options: EditorCommandsOptions = {},
): Promise<SaveTranscriptSessionResult> {
  const { invokeFn = defaultInvoke } = options;
  // Keys stay snake_case; host commands use `rename_all = "snake_case"`.
  return invokeFn<SaveTranscriptSessionResult>("save_transcript_session", { ...request });
}

export async function getEditorSettings(
  options: EditorCommandsOptions = {},
): Promise<GetEditorSettingsResponse> {
  const { invokeFn = defaultInvoke } = options;
  return invokeFn<GetEditorSettingsResponse>("get_editor_settings");
}

export async function setEditorSettings(
  request: SetEditorSettingsRequest,
  options: EditorCommandsOptions = {},
): Promise<SetEditorSettingsResponse> {
  const { invokeFn = defaultInvoke } = options;
  return invokeFn<SetEditorSettingsResponse>("set_editor_settings", { ...request });
}

export async function pickSaveDirectory(
  options: EditorCommandsOptions = {},
): Promise<PickSaveDirectoryResponse> {
  const { invokeFn = defaultInvoke } = options;
  return invokeFn<PickSaveDirectoryResponse>("pick_save_directory");
}
