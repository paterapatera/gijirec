import type { AiTranscriptionJsonlRecord } from "./export";

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
