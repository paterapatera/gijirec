import type { CaptureSessionCapturePhase, CaptureSessionPhase } from "./capture-session-types";
import type { CapturePhaseChanged } from "./capture-status";

export type CapturePhase = CapturePhaseChanged["phase"];

/** Capture phase for session-aware gates (audio controls, req 3.1–3.2). */
export function resolveCapturePhaseForGate(options: {
  override?: CapturePhase;
  sessionSubscribed: boolean;
  sessionPhase: CaptureSessionPhase;
  sessionCapturePhase: CaptureSessionCapturePhase;
  legacyPhase: CapturePhase;
}): CapturePhase {
  if (options.override !== undefined) {
    return options.override;
  }
  if (!options.sessionSubscribed || options.sessionPhase === "idle") {
    return options.legacyPhase;
  }
  if (options.legacyPhase !== "idle") {
    return options.legacyPhase;
  }
  return options.sessionCapturePhase;
}

/** Capture phase label when capture-session toggle owns the lifecycle. */
export function resolveDisplayedCapturePhase(
  sessionPhase: CaptureSessionPhase,
  sessionCapturePhase: CaptureSessionCapturePhase,
  legacyPhase: CapturePhase,
): CapturePhase {
  if (sessionPhase === "idle") {
    return legacyPhase;
  }
  return sessionCapturePhase;
}
