import type { Range } from "slate";

/** Upstream whisper-transcribe block payload (snake_case contract mirror). */
export interface TranscriptBlockContract {
  block_id: string;
  sequence: number;
  text: string;
  start_timestamp_ms: number;
  language: string;
}

/** Domain view of an appended transcript block (camelCase). */
export interface TranscriptBlockView {
  blockId: string;
  sequence: number;
  text: string;
  startTimestampMs: number;
  language: string;
  displayText: string;
}

/** transcript-editor-settings.md contract mirror. */
export interface EditorSettings {
  save_directory: string | null;
  export_jsonl_enabled: boolean;
}

export const DEFAULT_EDITOR_SETTINGS: EditorSettings = {
  save_directory: null,
  export_jsonl_enabled: false,
};

/** Slate selection range associated with a transcript block for partial lock. */
export interface LockRange {
  range: Range;
  blockId: string;
}

export function mapBlockFromContract(block: TranscriptBlockContract): TranscriptBlockView {
  return {
    blockId: block.block_id,
    sequence: block.sequence,
    text: block.text,
    startTimestampMs: block.start_timestamp_ms,
    language: block.language,
    displayText: block.text,
  };
}
