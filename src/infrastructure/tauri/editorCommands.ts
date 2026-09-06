import { invoke } from "@tauri-apps/api/core";
import type { AiTranscriptionJsonlRecord } from "../../domain/transcript/export";
import type { EditorSettings } from "../../domain/transcript/types";

/** transcript-editor-status.md contract mirror. */
type EditorUserErrorCode =
  | "SAVE_DIRECTORY_NOT_SET"
  | "SAVE_DIRECTORY_UNAVAILABLE"
  | "SAVE_DIRECTORY_CREATE_FAILED"
  | "SAVE_FILE_WRITE_FAILED"
  | "SAVE_PARTIAL_FAILURE"
  | "SETTINGS_PERSIST_FAILED"
  | "INTERNAL";

interface EditorUserError {
  code: EditorUserErrorCode;
  message_ja: string;
  action_ja: string;
  recoverable: boolean;
}

/** transcript-editor-save.md contract mirror. */
export interface SaveTranscriptSessionRequest {
  session_id: string;
  handwriting_markdown: string;
  ai_transcription_markdown: string;
  ai_transcription_jsonl?: AiTranscriptionJsonlRecord[];
}

export interface SaveTranscriptSessionResult {
  success: boolean;
  output_directory?: string;
  files_written?: string[];
  files_failed?: Array<{ path: string; reason_ja: string }>;
  error?: EditorUserError;
}

/** transcript-editor-settings.md contract mirror. */
export interface SetEditorSettingsRequest {
  save_directory?: string | null;
  export_jsonl_enabled?: boolean;
}

export type GetEditorSettingsResponse = EditorSettings;
export type SetEditorSettingsResponse = EditorSettings;
export type PickSaveDirectoryResponse = string | null;

export interface EditorCommandsOptions {
  invokeFn?: typeof invoke;
}

export async function saveTranscriptSession(
  request: SaveTranscriptSessionRequest,
  options: EditorCommandsOptions = {},
): Promise<SaveTranscriptSessionResult> {
  const { invokeFn = invoke } = options;
  // Keys stay snake_case; host commands use `rename_all = "snake_case"`.
  return invokeFn<SaveTranscriptSessionResult>("save_transcript_session", { ...request });
}

export async function getEditorSettings(
  options: EditorCommandsOptions = {},
): Promise<GetEditorSettingsResponse> {
  const { invokeFn = invoke } = options;
  return invokeFn<GetEditorSettingsResponse>("get_editor_settings");
}

export async function setEditorSettings(
  request: SetEditorSettingsRequest,
  options: EditorCommandsOptions = {},
): Promise<SetEditorSettingsResponse> {
  const { invokeFn = invoke } = options;
  return invokeFn<SetEditorSettingsResponse>("set_editor_settings", { ...request });
}

export async function pickSaveDirectory(
  options: EditorCommandsOptions = {},
): Promise<PickSaveDirectoryResponse> {
  const { invokeFn = invoke } = options;
  return invokeFn<PickSaveDirectoryResponse>("pick_save_directory");
}
