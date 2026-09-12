import { listen } from "@tauri-apps/api/event";
import type { Dispatch, SetStateAction } from "react";
import {
  defaultInvoke,
  type InjectableInvokeFn,
} from "../../infrastructure/tauri/injectableInvoke";
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
import { useTauriEventMirror } from "./useTauriEventMirror";

export interface UseTranscribeStatusOptions {
  listenFn?: TranscribeEventListenFn;
  invokeFn?: InjectableInvokeFn;
  /** When false, skips invoke sync and event subscriptions (default true). */
  enabled?: boolean;
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
  invokeFn: InjectableInvokeFn,
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

function cleanupTranscribeHandles(handles: {
  unlistenPhase: () => void;
  unlistenProgress: () => void;
  unlistenError: () => void;
}): void {
  handles.unlistenPhase();
  handles.unlistenProgress();
  handles.unlistenError();
}

export function useTranscribeStatus(
  options: UseTranscribeStatusOptions = {},
): TranscribeStatusState {
  const { listenFn = listen, invokeFn = defaultInvoke, enabled = true } = options;

  return useTauriEventMirror({
    enabled,
    initialState: INITIAL_TRANSCRIBE_STATUS,
    invokeFn,
    listenFn,
    syncInitial: syncInitialPhase,
    subscribeEvents: subscribeTranscribeEvents,
    cleanupHandles: cleanupTranscribeHandles,
  });
}
