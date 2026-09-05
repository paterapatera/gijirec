import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Dispatch, SetStateAction } from "react";
import { useEffect, useState } from "react";
import type {
  ModelDownloadProgress,
  TranscribeEventListenFn,
  TranscribePhaseChanged,
  TranscribeStatusState,
  TranscribeUserError,
} from "./transcribe-status";
import {
  INITIAL_TRANSCRIBE_STATUS,
  MODEL_PROGRESS_EVENT,
  PHASE_CHANGED_EVENT,
  TRANSCRIBE_ERROR_EVENT,
} from "./transcribe-status";

export interface UseTranscribeStatusOptions {
  listenFn?: TranscribeEventListenFn;
  invokeFn?: typeof invoke;
}

function applyPhaseChanged(
  payload: TranscribePhaseChanged,
  setStatus: Dispatch<SetStateAction<TranscribeStatusState>>,
): void {
  const { phase, timestamp_ms: timestampMs } = payload;
  setStatus((prev) => ({
    ...prev,
    phase,
    timestampMs,
    error: phase === "error" ? prev.error : null,
  }));
}

function applyModelProgress(
  payload: ModelDownloadProgress,
  setStatus: Dispatch<SetStateAction<TranscribeStatusState>>,
): void {
  setStatus((prev) => ({
    ...prev,
    phase:
      prev.phase === "idle" && (payload.status === "downloading" || payload.status === "verifying")
        ? "loading_model"
        : prev.phase,
    modelProgress: payload,
  }));
}

function applyError(
  payload: TranscribeUserError,
  setStatus: Dispatch<SetStateAction<TranscribeStatusState>>,
): void {
  setStatus((prev) => ({
    ...prev,
    phase: "error",
    error: payload,
  }));
}

interface TranscribeStatusSnapshot {
  phase: TranscribePhaseChanged;
  model_progress: ModelDownloadProgress | null;
}

async function syncInitialPhase(
  invokeFn: typeof invoke,
  setStatus: Dispatch<SetStateAction<TranscribeStatusState>>,
): Promise<void> {
  try {
    const snapshot = await invokeFn<TranscribeStatusSnapshot>("get_transcribe_status");
    applyPhaseChanged(snapshot.phase, setStatus);
    if (snapshot.model_progress !== null) {
      applyModelProgress(snapshot.model_progress, setStatus);
    }
  } catch {
    // Browser-only dev (no Tauri shell) — keep idle until events arrive.
  }
}

async function subscribeTranscribeEvents(
  listenFn: TranscribeEventListenFn,
  setStatus: Dispatch<SetStateAction<TranscribeStatusState>>,
  isCancelled: () => boolean,
): Promise<
  | {
      unlistenPhase: () => void;
      unlistenProgress: () => void;
      unlistenError: () => void;
    }
  | undefined
> {
  const unlistenPhase = await listenFn(PHASE_CHANGED_EVENT, (event) => {
    applyPhaseChanged(event.payload as TranscribePhaseChanged, setStatus);
  });
  if (isCancelled()) {
    unlistenPhase();
    return undefined;
  }

  const unlistenProgress = await listenFn(MODEL_PROGRESS_EVENT, (event) => {
    applyModelProgress(event.payload as ModelDownloadProgress, setStatus);
  });
  if (isCancelled()) {
    unlistenPhase();
    unlistenProgress();
    return undefined;
  }

  const unlistenError = await listenFn(TRANSCRIBE_ERROR_EVENT, (event) => {
    applyError(event.payload as TranscribeUserError, setStatus);
  });
  if (isCancelled()) {
    unlistenPhase();
    unlistenProgress();
    unlistenError();
    return undefined;
  }

  return { unlistenPhase, unlistenProgress, unlistenError };
}

export function useTranscribeStatus(
  options: UseTranscribeStatusOptions = {},
): TranscribeStatusState {
  const { listenFn = listen, invokeFn = invoke } = options;
  const [status, setStatus] = useState<TranscribeStatusState>(INITIAL_TRANSCRIBE_STATUS);

  useEffect(() => {
    let cancelled = false;
    let cleanupFns:
      | {
          unlistenPhase: () => void;
          unlistenProgress: () => void;
          unlistenError: () => void;
        }
      | undefined;

    void syncInitialPhase(invokeFn, setStatus);
    void subscribeTranscribeEvents(listenFn, setStatus, () => cancelled).then((handles) => {
      if (handles === undefined) {
        return;
      }
      cleanupFns = handles;
      if (cancelled) {
        cleanupFns.unlistenPhase();
        cleanupFns.unlistenProgress();
        cleanupFns.unlistenError();
      }
    });

    return () => {
      cancelled = true;
      if (cleanupFns) {
        cleanupFns.unlistenPhase();
        cleanupFns.unlistenProgress();
        cleanupFns.unlistenError();
      }
    };
  }, [invokeFn, listenFn]);

  return status;
}
