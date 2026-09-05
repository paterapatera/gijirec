import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Dispatch, SetStateAction } from "react";
import { useEffect, useState } from "react";
import type {
  CaptureEventListenFn,
  CapturePhaseChanged,
  CaptureStatusState,
  CaptureUserError,
} from "./capture-status";
import { ERROR_EVENT, INITIAL_CAPTURE_STATUS, PHASE_CHANGED_EVENT } from "./capture-status";

export interface UseCaptureStatusOptions {
  listenFn?: CaptureEventListenFn;
  invokeFn?: typeof invoke;
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
  invokeFn: typeof invoke,
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

/**
 * Subscribes to capture lifecycle Tauri events and mirrors phase/error into React state.
 * Unlistens on unmount (req 5.4 — surfaces action_ja for UI in task 6.2).
 */
export function useCaptureStatus(options: UseCaptureStatusOptions = {}): CaptureStatusState {
  const { listenFn = listen, invokeFn = invoke } = options;
  const [status, setStatus] = useState<CaptureStatusState>(INITIAL_CAPTURE_STATUS);

  useEffect(() => {
    let cancelled = false;
    let cleanupListeners: (() => void) | undefined;

    void syncInitialPhase(invokeFn, setStatus);
    void subscribeCaptureEvents(listenFn, setStatus, () => cancelled).then((handles) => {
      if (handles === undefined) {
        return;
      }
      cleanupListeners = () => {
        handles.unlistenPhase();
        handles.unlistenError();
      };
      if (cancelled) {
        cleanupListeners();
      }
    });

    return () => {
      cancelled = true;
      cleanupListeners?.();
    };
  }, [invokeFn, listenFn]);

  return status;
}
