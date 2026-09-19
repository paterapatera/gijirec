/** Contract types per `docs/contracts/capture-session-toggle.md`. */

export const CAPTURE_SESSION_STATE_CHANGED_EVENT = "capture-session://state-changed" as const;

export type CaptureSessionPhase = "idle" | "starting" | "active";

export type CaptureSessionCapturePhase = "idle" | "starting" | "capturing" | "stopping" | "error";

export interface CaptureSessionState {
  session_phase: CaptureSessionPhase;
  transition_busy: boolean;
  capture_phase: CaptureSessionCapturePhase;
  timestamp_ms: number;
}

export interface CaptureSessionStateChanged {
  state: CaptureSessionState;
}

export type CaptureSessionErrorCode =
  | "TRANSITION_BUSY"
  | "CAPTURE_START_FAILED"
  | "UNSUPPORTED_PLATFORM"
  | "INTERNAL";

export interface CaptureSessionInvokeError {
  code: CaptureSessionErrorCode;
  message_ja: string;
  action_ja: string;
}

export interface CaptureSessionHookState {
  session_phase: CaptureSessionPhase;
  transition_busy: boolean;
  capture_phase: CaptureSessionCapturePhase;
  timestamp_ms: number;
  /** true when `transition_busy` (req 2.6 — block double start). */
  disabled: boolean;
  /** mirrors `transition_busy` for `aria-busy`. */
  busy: boolean;
}

export const INITIAL_CAPTURE_SESSION_HOOK_STATE: CaptureSessionHookState = {
  session_phase: "idle",
  transition_busy: false,
  capture_phase: "idle",
  timestamp_ms: 0,
  disabled: false,
  busy: false,
};

type CaptureSessionEventHandler = (event: { payload: CaptureSessionStateChanged }) => void;

export type CaptureSessionEventListenFn = (
  event: string,
  handler: CaptureSessionEventHandler,
) => Promise<() => void>;
