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
import type { CaptureEventListenFn, CapturePhaseChanged } from "./capture-status";
import { useCaptureStatus } from "./useCaptureStatus";

type CapturePhase = CapturePhaseChanged["phase"];

export interface UseCaptureAudioControlsOptions {
  listenFn?: CaptureAudioControlsEventListenFn;
  invokeFn?: InjectableInvokeFn;
  /**
   * When set, overrides capture phase for the disabled gate (req 1.6 / 3.5).
   * When omitted, phase comes from CaptureStatusProvider or `useCaptureStatus`.
   */
  capturePhase?: CapturePhase;
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
  phase: CapturePhase,
  setState: Dispatch<SetStateAction<CaptureAudioControlsHookState>>,
): void {
  setState((prev) => ({
    ...prev,
    disabled: phase !== "capturing",
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

/**
 * Mirrors capture audio controls from Tauri commands and control/meter events.
 * Controls are disabled when capture phase is not `capturing` (req 1.6 / 3.5).
 */
export function useCaptureAudioControls(
  options: UseCaptureAudioControlsOptions = {},
): CaptureAudioControlsHookState {
  const {
    listenFn = listen,
    invokeFn = defaultInvoke,
    capturePhase: capturePhaseOverride,
  } = options;
  const contextStatus = useOptionalCaptureStatusContext();
  const needsCaptureSubscription = capturePhaseOverride === undefined && contextStatus === null;
  const subscribedStatus = useCaptureStatus({
    invokeFn,
    listenFn: listenFn as CaptureEventListenFn,
    enabled: needsCaptureSubscription,
  });
  const phase = capturePhaseOverride ?? contextStatus?.phase ?? subscribedStatus.phase;
  const [state, setState] = useState<CaptureAudioControlsHookState>(
    INITIAL_CAPTURE_AUDIO_CONTROLS_HOOK_STATE,
  );

  useEffect(() => {
    applyDisabled(phase, setState);
  }, [phase]);

  useEffect(() => {
    let cancelled = false;
    let cleanupListeners: (() => void) | undefined;

    void syncInitialControls(invokeFn, setState);
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
  }, [invokeFn, listenFn]);

  return state;
}
