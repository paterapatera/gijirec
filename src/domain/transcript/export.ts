import type { TranscriptBlockView } from "./types";

/** transcript-editor-save.md contract mirror. */
export interface AiTranscriptionJsonlRecord {
  block_id: string;
  sequence: number;
  text: string;
  start_timestamp_ms: number;
  language: string;
}

/** Serializes AI transcript blocks to plain Markdown text without timestamps. */
export function toAiMarkdown(blocks: readonly TranscriptBlockView[]): string {
  if (blocks.length === 0) {
    return "";
  }

  return blocks.map((block) => block.displayText).join("\n");
}

/** Serializes AI transcript blocks to JSONL contract records. */
export function toJsonlRecords(
  blocks: readonly TranscriptBlockView[],
): AiTranscriptionJsonlRecord[] {
  return blocks.map((block) => ({
    block_id: block.blockId,
    sequence: block.sequence,
    text: block.displayText,
    start_timestamp_ms: block.startTimestampMs,
    language: block.language,
  }));
}
