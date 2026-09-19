import type { InjectableInvokeFn } from "../../infrastructure/tauri/injectableInvoke";
import type { CaptureSessionEventListenFn } from "../hooks/capture-session-types";
import type { CaptureEventListenFn, CaptureStatusState } from "../hooks/capture-status";
import type { TranscribeStatusState } from "../hooks/transcribe-status";
import { useDisplayedCapturePhase } from "../hooks/useDisplayedCapturePhase";
import { AppCaptureStatusHeader } from "./AppCaptureStatusHeader";
import { DeviceSelectorPanel } from "./DeviceSelectorPanel";

export interface AppCaptureSessionBarProps {
  readonly captureStatus: CaptureStatusState;
  readonly transcribeStatus: TranscribeStatusState;
  readonly invokeFn: InjectableInvokeFn;
  readonly listenFn?: CaptureEventListenFn;
}

export function AppCaptureSessionBar({
  captureStatus,
  transcribeStatus,
  invokeFn,
  listenFn,
}: AppCaptureSessionBarProps) {
  const sessionListenFn = listenFn as CaptureSessionEventListenFn | undefined;
  const capturePhase = useDisplayedCapturePhase({
    legacyPhase: captureStatus.phase,
    invokeFn,
    ...(sessionListenFn !== undefined ? { listenFn: sessionListenFn } : {}),
  });

  return (
    <>
      <AppCaptureStatusHeader
        captureStatus={captureStatus}
        transcribeStatus={transcribeStatus}
        invokeFn={invokeFn}
        {...(sessionListenFn !== undefined ? { listenFn: sessionListenFn } : {})}
      />
      <DeviceSelectorPanel
        invokeFn={invokeFn}
        capturePhase={capturePhase}
        captureError={captureStatus.error}
        {...(listenFn !== undefined ? { listenFn } : {})}
      />
    </>
  );
}
