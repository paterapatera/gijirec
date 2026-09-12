import { listen } from "@tauri-apps/api/event";
import type { Dispatch, SetStateAction } from "react";
import {
  defaultInvoke,
  type InjectableInvokeFn,
} from "../../infrastructure/tauri/injectableInvoke";
import type {
  CaptureEventListenFn,
  CapturePhaseChanged,
  CaptureStatusState,
  CaptureUserError,
} from "./capture-status";
import { ERROR_EVENT, INITIAL_CAPTURE_STATUS, PHASE_CHANGED_EVENT } from "./capture-status";
import { useTauriEventMirror } from "./useTauriEventMirror";

export interface UseCaptureStatusOptions {
  listenFn?: CaptureEventListenFn;
  invokeFn?: InjectableInvokeFn;
  /** When false, skips invoke sync and event subscriptions (default true). */
  enabled?: boolean;
}

function applyPhaseChanged(
  payload: CapturePhaseChanged,
  setStatus: Dispatch<SetStateAction<CaptureStatusState>>,
): void {
  const { phase, timestamp_ms: timestampMs } = payload;
  setStatus((prev) => ({
    phase,
    timestampMs,
    error: phase === "error" ? prev.error : null,
  }));
}

function applyError(payload: CaptureUserError): CaptureStatusState {
  return {
    phase: "error",
    timestampMs: null,
    error: payload,
  };
}

async function syncInitialPhase(
  invokeFn: InjectableInvokeFn,
  setStatus: Dispatch<SetStateAction<CaptureStatusState>>,
): Promise<void> {
  try {
    const payload = await invokeFn<CapturePhaseChanged>("get_capture_phase");
    applyPhaseChanged(payload, setStatus);
  } catch {
    // Browser-only dev (no Tauri shell) — keep idle until events arrive.
  }
}

async function subscribeCaptureEvents(
  listenFn: CaptureEventListenFn,
  setStatus: Dispatch<SetStateAction<CaptureStatusState>>,
  isCancelled: () => boolean,
): Promise<{ unlistenPhase: () => void; unlistenError: () => void } | undefined> {
  const unlistenPhase = await listenFn(PHASE_CHANGED_EVENT, (event) => {
    applyPhaseChanged(event.payload as CapturePhaseChanged, setStatus);
  });
  if (isCancelled()) {
    unlistenPhase();
    return undefined;
  }

  const unlistenError = await listenFn(ERROR_EVENT, (event) => {
    setStatus(applyError(event.payload as CaptureUserError));
  });
  if (isCancelled()) {
    unlistenPhase();
    unlistenError();
    return undefined;
  }

  return { unlistenPhase, unlistenError };
}

function cleanupCaptureHandles(handles: {
  unlistenPhase: () => void;
  unlistenError: () => void;
}): void {
  handles.unlistenPhase();
  handles.unlistenError();
}

/**
 * Subscribes to capture lifecycle Tauri events and mirrors phase/error into React state.
 * Unlistens on unmount (req 5.4 — surfaces action_ja for UI in task 6.2).
 */
export function useCaptureStatus(options: UseCaptureStatusOptions = {}): CaptureStatusState {
  const { listenFn = listen, invokeFn = defaultInvoke, enabled = true } = options;

  return useTauriEventMirror({
    enabled,
    initialState: INITIAL_CAPTURE_STATUS,
    invokeFn,
    listenFn,
    syncInitial: syncInitialPhase,
    subscribeEvents: subscribeCaptureEvents,
    cleanupHandles: cleanupCaptureHandles,
  });
}
