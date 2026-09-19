import { listen } from "@tauri-apps/api/event";
import type { Dispatch, SetStateAction } from "react";
import { useEffect, useState } from "react";
import { getCaptureAudioControls } from "../../infrastructure/tauri/captureAudioControlsCommands";
import {
  defaultInvoke,
  type InjectableInvokeFn,
} from "../../infrastructure/tauri/injectableInvoke";
import { useOptionalCaptureStatusContext } from "./CaptureStatusContext";
import type {
  CaptureAudioControlsChanged,
  CaptureAudioControlsEventListenFn,
  CaptureAudioControlsHookState,
  CaptureAudioControlsState,
  IngestLevelChanged,
} from "./capture-audio-controls-types";
import {
  CONTROLS_CHANGED_EVENT,
  INGEST_LEVEL_EVENT,
  INITIAL_CAPTURE_AUDIO_CONTROLS_HOOK_STATE,
} from "./capture-audio-controls-types";
import { type CapturePhase, resolveCapturePhaseForGate } from "./capture-phase-gate";
import type { CaptureSessionEventListenFn, CaptureSessionPhase } from "./capture-session-types";
import type { CaptureEventListenFn } from "./capture-status";
import { useCaptureSession } from "./useCaptureSession";
import { useCaptureStatus } from "./useCaptureStatus";

export function resolveCaptureAudioControlsDisabled(
  sessionPhase: CaptureSessionPhase,
  capturePhase: CapturePhase,
): boolean {
  return sessionPhase !== "active" || capturePhase !== "capturing";
}

export interface UseCaptureAudioControlsOptions {
  listenFn?: CaptureAudioControlsEventListenFn;
  invokeFn?: InjectableInvokeFn;
  /**
   * When set, overrides capture phase for the disabled gate (req 1.6 / 3.5).
   * When omitted, phase comes from CaptureStatusProvider or `useCaptureStatus`.
   */
  capturePhase?: CapturePhase;
  /**
   * When set, overrides session phase for the disabled gate (req 3.1 / 3.2).
   * When omitted, phase comes from `useCaptureSession`.
   */
  sessionPhase?: CaptureSessionPhase;
}

function isCaptureAudioControlsState(value: unknown): value is CaptureAudioControlsState {
  if (value === null || typeof value !== "object") {
    return false;
  }
  const record = value as { controls?: unknown; ingest_level?: unknown };
  if (record.controls === null || typeof record.controls !== "object") {
    return false;
  }
  const controls = record.controls as Record<string, unknown>;
  return (
    typeof controls.mic_ingest_enabled === "boolean" &&
    typeof controls.manual_ingest_gain === "number" &&
    typeof controls.gain_user_adjusted === "boolean"
  );
}

function applyControlsChanged(
  payload: CaptureAudioControlsChanged,
  setState: Dispatch<SetStateAction<CaptureAudioControlsHookState>>,
): void {
  setState((prev) => ({
    ...prev,
    controls: payload.controls,
  }));
}

function applyIngestLevelChanged(
  payload: IngestLevelChanged,
  setState: Dispatch<SetStateAction<CaptureAudioControlsHookState>>,
): void {
  setState((prev) => ({
    ...prev,
    ingest_level: {
      level_dbfs: payload.level_dbfs,
      timestamp_ms: payload.timestamp_ms,
    },
  }));
}

function applyDisabled(
  sessionPhase: CaptureSessionPhase,
  capturePhase: CapturePhase,
  setState: Dispatch<SetStateAction<CaptureAudioControlsHookState>>,
): void {
  setState((prev) => ({
    ...prev,
    disabled: resolveCaptureAudioControlsDisabled(sessionPhase, capturePhase),
  }));
}

async function syncInitialControls(
  invokeFn: InjectableInvokeFn,
  setState: Dispatch<SetStateAction<CaptureAudioControlsHookState>>,
): Promise<void> {
  try {
    const stateRaw: unknown = await getCaptureAudioControls({ invokeFn });
    if (!isCaptureAudioControlsState(stateRaw)) {
      return;
    }
    setState((prev) => ({
      ...prev,
      controls: stateRaw.controls,
      ingest_level: stateRaw.ingest_level,
    }));
  } catch (error) {
    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      console.error("get_capture_audio_controls failed", error);
    }
  }
}

async function subscribeCaptureAudioControlsEvents(
  listenFn: CaptureAudioControlsEventListenFn,
  setState: Dispatch<SetStateAction<CaptureAudioControlsHookState>>,
  isCancelled: () => boolean,
): Promise<{ unlistenControls: () => void; unlistenLevel: () => void } | undefined> {
  const unlistenControls = await listenFn(CONTROLS_CHANGED_EVENT, (event) => {
    applyControlsChanged(event.payload as CaptureAudioControlsChanged, setState);
  });
  if (isCancelled()) {
    unlistenControls();
    return undefined;
  }

  const unlistenLevel = await listenFn(INGEST_LEVEL_EVENT, (event) => {
    applyIngestLevelChanged(event.payload as IngestLevelChanged, setState);
  });
  if (isCancelled()) {
    unlistenControls();
    unlistenLevel();
    return undefined;
  }

  return { unlistenControls, unlistenLevel };
}

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function canSubscribeInRuntime(hasInjectableListen: boolean): boolean {
  return hasInjectableListen || isTauriRuntime();
}

function useControlGatePhases(options: UseCaptureAudioControlsOptions): {
  capturePhase: CapturePhase;
  sessionPhase: CaptureSessionPhase;
  listenFn: CaptureAudioControlsEventListenFn;
  invokeFn: InjectableInvokeFn;
  hasInjectableListen: boolean;
  canSubscribeAudioEvents: boolean;
} {
  const {
    listenFn: listenFnFromOptions,
    invokeFn = defaultInvoke,
    capturePhase: capturePhaseOverride,
    sessionPhase: sessionPhaseOverride,
  } = options;
  const listenFn = listenFnFromOptions ?? listen;
  const contextStatus = useOptionalCaptureStatusContext();
  const needsCaptureSubscription = capturePhaseOverride === undefined && contextStatus === null;
  const subscribedStatus = useCaptureStatus({
    invokeFn,
    listenFn: listenFn as CaptureEventListenFn,
    enabled: needsCaptureSubscription,
  });
  const hasInjectableListen = listenFnFromOptions !== undefined;
  const canSubscribeSession =
    sessionPhaseOverride === undefined && canSubscribeInRuntime(hasInjectableListen);
  const subscribedSession = useCaptureSession({
    invokeFn,
    ...(listenFnFromOptions !== undefined
      ? {
          listenFn: listenFnFromOptions as unknown as CaptureSessionEventListenFn,
        }
      : {}),
    enabled: canSubscribeSession,
  });
  const sessionPhase =
    sessionPhaseOverride ?? (canSubscribeSession ? subscribedSession.session_phase : "idle");
  const legacyCapturePhase = contextStatus?.phase ?? subscribedStatus.phase;
  const capturePhase = resolveCapturePhaseForGate({
    ...(capturePhaseOverride !== undefined ? { override: capturePhaseOverride } : {}),
    sessionSubscribed: canSubscribeSession,
    sessionPhase,
    sessionCapturePhase: subscribedSession.capture_phase,
    legacyPhase: legacyCapturePhase,
  });
  const canSubscribeAudioEvents = canSubscribeInRuntime(hasInjectableListen);
  return {
    capturePhase,
    sessionPhase,
    listenFn,
    invokeFn,
    hasInjectableListen,
    canSubscribeAudioEvents,
  };
}

/**
 * Mirrors capture audio controls from Tauri commands and control/meter events.
 * Controls are disabled unless session is `active` and capture is `capturing` (req 3.1 / 3.2).
 */
export function useCaptureAudioControls(
  options: UseCaptureAudioControlsOptions = {},
): CaptureAudioControlsHookState {
  const { capturePhase, sessionPhase, listenFn, invokeFn, canSubscribeAudioEvents } =
    useControlGatePhases(options);
  const [state, setState] = useState<CaptureAudioControlsHookState>(
    INITIAL_CAPTURE_AUDIO_CONTROLS_HOOK_STATE,
  );

  useEffect(() => {
    applyDisabled(sessionPhase, capturePhase, setState);
  }, [sessionPhase, capturePhase]);

  useEffect(() => {
    let cancelled = false;
    let cleanupListeners: (() => void) | undefined;

    void syncInitialControls(invokeFn, setState);
    if (!canSubscribeAudioEvents) {
      return;
    }

    void subscribeCaptureAudioControlsEvents(listenFn, setState, () => cancelled).then(
      (handles) => {
        if (handles === undefined) {
          return;
        }
        cleanupListeners = () => {
          handles.unlistenControls();
          handles.unlistenLevel();
        };
        if (cancelled) {
          cleanupListeners();
        }
      },
    );

    return () => {
      cancelled = true;
      cleanupListeners?.();
    };
  }, [canSubscribeAudioEvents, invokeFn, listenFn]);

  return state;
}
