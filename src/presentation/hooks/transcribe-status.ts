/** Contract types per `docs/contracts/whisper-transcribe-status.md`. */

export const PHASE_CHANGED_EVENT = "whisper-transcribe://phase-changed" as const;
export const MODEL_PROGRESS_EVENT = "whisper-transcribe://model-progress" as const;
export const TRANSCRIBE_ERROR_EVENT = "whisper-transcribe://error" as const;

export type TranscribePhase = "idle" | "loading_model" | "ready" | "transcribing" | "stopping" | "error";

export interface TranscribePhaseChanged {
  phase: TranscribePhase;
  timestamp_ms: number;
}

type ModelDownloadStatus = "downloading" | "verifying" | "complete" | "failed";

export interface ModelDownloadProgress {
  bytes_downloaded: number;
  bytes_total: number | null;
  percent: number | null;
  status: ModelDownloadStatus;
}

type TranscribeUserErrorCode =
  | "MODEL_DOWNLOAD_FAILED"
  | "MODEL_CORRUPT"
  | "MODEL_NOT_FOUND"
  | "INFERENCE_FAILED"
  | "UPSTREAM_CAPTURE_ERROR"
  | "INTERNAL";

export interface TranscribeUserError {
  code: TranscribeUserErrorCode;
  message_ja: string;
  action_ja: string;
  recoverable: boolean;
}

export interface TranscribeStatusState {
  phase: TranscribePhase;
  timestampMs: number | null;
  modelProgress: ModelDownloadProgress | null;
  error: TranscribeUserError | null;
}

export const INITIAL_TRANSCRIBE_STATUS: TranscribeStatusState = {
  phase: "idle",
  timestampMs: null,
  modelProgress: null,
  error: null,
};

type TranscribeEventHandler = (event: {
  payload: TranscribePhaseChanged | ModelDownloadProgress | TranscribeUserError;
}) => void;

export type TranscribeEventListenFn = (
  event: string,
  handler: TranscribeEventHandler,
) => Promise<() => void>;
