/** Contract types per `docs/contracts/whisper-transcribe-blocks.md`. */

export const BLOCK_APPENDED_EVENT = "whisper-transcribe://block-appended" as const;

export interface TranscriptBlockContract {
  block_id: string;
  sequence: number;
  text: string;
  start_timestamp_ms: number;
  language: string;
}

export interface TranscriptBlockAppended {
  block: TranscriptBlockContract;
  timestamp_ms: number;
}

type TranscriptBlockEventHandler = (event: { payload: unknown }) => void;

export type TranscriptBlockEventListenFn = (
  event: string,
  handler: TranscriptBlockEventHandler,
) => Promise<() => void>;
