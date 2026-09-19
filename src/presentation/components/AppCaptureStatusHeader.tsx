import type { InjectableInvokeFn } from "../../infrastructure/tauri/injectableInvoke";
import { resolveDisplayedCapturePhase } from "../hooks/capture-phase-gate";
import type { CaptureSessionEventListenFn } from "../hooks/capture-session-types";
import type { CaptureStatusState } from "../hooks/capture-status";
import type { TranscribeStatusState } from "../hooks/transcribe-status";
import { useCaptureSession } from "../hooks/useCaptureSession";
import { AppStatusPanels } from "./AppStatusPanels";
import { CaptureSessionStartControl } from "./CaptureSessionStartControl";

export interface AppCaptureStatusHeaderProps {
  readonly captureStatus: CaptureStatusState;
  readonly transcribeStatus: TranscribeStatusState;
  readonly invokeFn: InjectableInvokeFn;
  readonly listenFn?: CaptureSessionEventListenFn;
}

export function AppCaptureStatusHeader({
  captureStatus,
  transcribeStatus,
  invokeFn,
  listenFn,
}: AppCaptureStatusHeaderProps) {
  const session = useCaptureSession({
    invokeFn,
    ...(listenFn !== undefined ? { listenFn } : {}),
  });
  const capturePhase = resolveDisplayedCapturePhase(
    session.session_phase,
    session.capture_phase,
    captureStatus.phase,
  );

  return (
    <>
      <CaptureSessionStartControl
        session_phase={session.session_phase}
        disabled={session.disabled}
        busy={session.busy}
        startCaptureSession={session.startCaptureSession}
      />
      <AppStatusPanels
        capturePhase={capturePhase}
        captureError={captureStatus.error}
        transcribePhase={transcribeStatus.phase}
        transcribeError={transcribeStatus.error}
        modelProgress={transcribeStatus.modelProgress}
        pcmBacklogSeconds={transcribeStatus.pcmBacklogSeconds}
      />
    </>
  );
}
