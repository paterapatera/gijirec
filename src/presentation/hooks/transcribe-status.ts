/** Contract types per `docs/contracts/whisper-transcribe-status.md`. */

export const PHASE_CHANGED_EVENT = "whisper-transcribe://phase-changed" as const;
export const MODEL_PROGRESS_EVENT = "whisper-transcribe://model-progress" as const;
export const TRANSCRIBE_ERROR_EVENT = "whisper-transcribe://error" as const;
export const PCM_BACKLOG_EVENT = "whisper-transcribe://pcm-backlog" as const;

/** Hide backlog hint below this deque depth (seconds @ 16 kHz). */
const PCM_BACKLOG_DISPLAY_THRESHOLD_SECONDS = 30;

export type TranscribePhase =
  | "idle"
  | "loading_model"
  | "ready"
  | "transcribing"
  | "stopping"
  | "error";

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

export interface TranscribePcmBacklog {
  backlog_seconds: number;
}

type TranscribeUserErrorCode =
  | "MODEL_DOWNLOAD_FAILED"
  | "MODEL_CORRUPT"
  | "MODEL_NOT_FOUND"
  | "INFERENCE_FAILED"
  | "UPSTREAM_CAPTURE_ERROR"
  | "PCM_RETENTION_LIMIT_EXCEEDED"
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
  pcmBacklogSeconds: number;
}

export const INITIAL_TRANSCRIBE_STATUS: TranscribeStatusState = {
  phase: "idle",
  timestampMs: null,
  modelProgress: null,
  error: null,
  pcmBacklogSeconds: 0,
};

/** User-facing label when backlog exceeds display threshold; null if hidden. */
export function formatInferenceBacklogLabel(backlogSeconds: number): string | null {
  if (backlogSeconds < PCM_BACKLOG_DISPLAY_THRESHOLD_SECONDS) {
    return null;
  }
  const minutes = Math.max(1, Math.ceil(backlogSeconds / 60));
  return `推論待ち 約 ${String(minutes)} 分`;
}

type TranscribeEventHandler = (event: {
  payload:
    | TranscribePhaseChanged
    | ModelDownloadProgress
    | TranscribeUserError
    | TranscribePcmBacklog;
}) => void;

export type TranscribeEventListenFn = (
  event: string,
  handler: TranscribeEventHandler,
) => Promise<() => void>;
