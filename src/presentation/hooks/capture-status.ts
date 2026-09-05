/** Contract types per `docs/contracts/audio-capture-status.md`. */

export const PHASE_CHANGED_EVENT = "audio-capture://phase-changed" as const;
export const ERROR_EVENT = "audio-capture://error" as const;

type CapturePhase = "idle" | "starting" | "capturing" | "stopping" | "error";

export interface CapturePhaseChanged {
  phase: CapturePhase;
  timestamp_ms: number;
}

type CaptureUserErrorCode =
  | "MIC_UNAVAILABLE"
  | "MIC_PERMISSION_DENIED"
  | "SYSTEM_AUDIO_UNAVAILABLE"
  | "SYSTEM_AUDIO_PERMISSION_DENIED"
  | "DEVICE_DISCONNECTED"
  | "INTERNAL";

export interface CaptureUserError {
  code: CaptureUserErrorCode;
  message_ja: string;
  action_ja: string;
  recoverable: boolean;
}

export interface CaptureStatusState {
  phase: CapturePhase;
  timestampMs: number | null;
  error: CaptureUserError | null;
}

export const INITIAL_CAPTURE_STATUS: CaptureStatusState = {
  phase: "idle",
  timestampMs: null,
  error: null,
};

type CaptureEventHandler = (event: { payload: CapturePhaseChanged | CaptureUserError }) => void;

export type CaptureEventListenFn = (
  event: string,
  handler: CaptureEventHandler,
) => Promise<() => void>;
