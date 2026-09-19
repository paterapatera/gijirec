import { listen } from "@tauri-apps/api/event";
import type { Dispatch, SetStateAction } from "react";
import { useCallback, useEffect, useState } from "react";
import {
  getCaptureSessionState,
  startCaptureSession as startCaptureSessionCommand,
} from "../../infrastructure/tauri/captureSessionCommands";
import {
  defaultInvoke,
  type InjectableInvokeFn,
} from "../../infrastructure/tauri/injectableInvoke";
import type {
  CaptureSessionCapturePhase,
  CaptureSessionEventListenFn,
  CaptureSessionHookState,
  CaptureSessionPhase,
  CaptureSessionState,
} from "./capture-session-types";
import {
  CAPTURE_SESSION_STATE_CHANGED_EVENT,
  INITIAL_CAPTURE_SESSION_HOOK_STATE,
} from "./capture-session-types";

export interface UseCaptureSessionOptions {
  listenFn?: CaptureSessionEventListenFn;
  invokeFn?: InjectableInvokeFn;
  /** When false, skips invoke sync and event subscriptions (default true). */
  enabled?: boolean;
}

export interface UseCaptureSessionResult extends CaptureSessionHookState {
  startCaptureSession: () => Promise<CaptureSessionState>;
}

function toHookState(state: Partial<CaptureSessionState>): CaptureSessionHookState {
  const normalized = normalizeCaptureSessionState(state);
  return {
    session_phase: normalized.session_phase,
    transition_busy: normalized.transition_busy,
    capture_phase: normalized.capture_phase,
    timestamp_ms: normalized.timestamp_ms,
    disabled: normalized.transition_busy,
    busy: normalized.transition_busy,
  };
}

const SESSION_PHASES: CaptureSessionPhase[] = ["idle", "starting", "active"];
const CAPTURE_PHASES: CaptureSessionCapturePhase[] = [
  "idle",
  "starting",
  "capturing",
  "stopping",
  "error",
];

function normalizeCaptureSessionState(state: Partial<CaptureSessionState>): CaptureSessionState {
  return {
    session_phase: SESSION_PHASES.includes(state.session_phase as CaptureSessionPhase)
      ? (state.session_phase as CaptureSessionPhase)
      : "idle",
    transition_busy: state.transition_busy ?? false,
    capture_phase: CAPTURE_PHASES.includes(state.capture_phase as CaptureSessionCapturePhase)
      ? (state.capture_phase as CaptureSessionCapturePhase)
      : "idle",
    timestamp_ms: typeof state.timestamp_ms === "number" ? state.timestamp_ms : 0,
  };
}

function applySessionState(
  state: Partial<CaptureSessionState>,
  setState: Dispatch<SetStateAction<CaptureSessionHookState>>,
): void {
  setState(toHookState(state));
}

async function syncInitialSession(
  invokeFn: InjectableInvokeFn,
  setState: Dispatch<SetStateAction<CaptureSessionHookState>>,
): Promise<void> {
  try {
    const state = await getCaptureSessionState({ invokeFn });
    applySessionState(state, setState);
  } catch {
    // Browser-only dev (no Tauri shell) — keep defaults until events arrive.
  }
}

async function subscribeCaptureSessionEvents(
  listenFn: CaptureSessionEventListenFn,
  setState: Dispatch<SetStateAction<CaptureSessionHookState>>,
  isCancelled: () => boolean,
): Promise<{ unlisten: () => void } | undefined> {
  const unlisten = await listenFn(CAPTURE_SESSION_STATE_CHANGED_EVENT, (event) => {
    applySessionState(event.payload.state, setState);
  });
  if (isCancelled()) {
    unlisten();
    return undefined;
  }
  return { unlisten };
}

/**
 * Mirrors capture session state from Tauri commands and session state-changed events.
 * `disabled` / `busy` follow `transition_busy` (req 2.6).
 */
export function useCaptureSession(options: UseCaptureSessionOptions = {}): UseCaptureSessionResult {
  const listenFnFromOptions = options.listenFn;
  const listenFn = listenFnFromOptions ?? listen;
  const { invokeFn = defaultInvoke, enabled = true } = options;
  const canSubscribeEvents =
    enabled &&
    (listenFnFromOptions !== undefined ||
      (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window));

  const [state, setState] = useState<CaptureSessionHookState>(INITIAL_CAPTURE_SESSION_HOOK_STATE);

  useEffect(() => {
    if (!enabled) {
      return;
    }

    let cancelled = false;
    let cleanupListeners: (() => void) | undefined;

    void syncInitialSession(invokeFn, setState);
    if (!canSubscribeEvents) {
      return;
    }

    void subscribeCaptureSessionEvents(listenFn, setState, () => cancelled).then((handles) => {
      if (handles === undefined) {
        return;
      }
      cleanupListeners = () => {
        handles.unlisten();
      };
      if (cancelled) {
        cleanupListeners();
      }
    });

    return () => {
      cancelled = true;
      cleanupListeners?.();
    };
  }, [canSubscribeEvents, enabled, invokeFn, listenFn]);

  const startCaptureSession = useCallback(async (): Promise<CaptureSessionState> => {
    const next = await startCaptureSessionCommand({ invokeFn });
    applySessionState(next, setState);
    return next;
  }, [invokeFn]);

  return {
    ...state,
    startCaptureSession,
  };
}
