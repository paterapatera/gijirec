import type { InjectableInvokeFn } from "../../infrastructure/tauri/injectableInvoke";
import { type CapturePhase, resolveDisplayedCapturePhase } from "./capture-phase-gate";
import type { CaptureSessionEventListenFn } from "./capture-session-types";
import type { CapturePhaseChanged } from "./capture-status";
import { useCaptureSession } from "./useCaptureSession";

export function useDisplayedCapturePhase(options: {
  legacyPhase: CapturePhaseChanged["phase"];
  invokeFn: InjectableInvokeFn;
  listenFn?: CaptureSessionEventListenFn;
}): CapturePhase {
  const session = useCaptureSession({
    invokeFn: options.invokeFn,
    ...(options.listenFn !== undefined ? { listenFn: options.listenFn } : {}),
  });
  return resolveDisplayedCapturePhase(
    session.session_phase,
    session.capture_phase,
    options.legacyPhase,
  );
}
