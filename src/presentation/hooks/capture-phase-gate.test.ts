import { describe, expect, test } from "bun:test";
import { resolveCapturePhaseForGate, resolveDisplayedCapturePhase } from "./capture-phase-gate";
import { resolveCaptureAudioControlsDisabled } from "./useCaptureAudioControls";

describe("capture-phase-gate", () => {
  test("resolveCapturePhaseForGate prefers session mirror when subscribed and session active", () => {
    expect(
      resolveCapturePhaseForGate({
        sessionSubscribed: true,
        sessionPhase: "active",
        sessionCapturePhase: "capturing",
        legacyPhase: "idle",
      }),
    ).toBe("capturing");
  });

  test("resolveCapturePhaseForGate prefers legacy audio-capture when subscribed and non-idle", () => {
    expect(
      resolveCapturePhaseForGate({
        sessionSubscribed: true,
        sessionPhase: "active",
        sessionCapturePhase: "starting",
        legacyPhase: "capturing",
      }),
    ).toBe("capturing");
  });

  test("resolveCapturePhaseForGate uses legacy when session not subscribed", () => {
    expect(
      resolveCapturePhaseForGate({
        sessionSubscribed: false,
        sessionPhase: "idle",
        sessionCapturePhase: "capturing",
        legacyPhase: "idle",
      }),
    ).toBe("idle");
  });

  test("resolveDisplayedCapturePhase uses session mirror while session is not idle", () => {
    expect(resolveDisplayedCapturePhase("active", "capturing", "idle")).toBe("capturing");
    expect(resolveDisplayedCapturePhase("starting", "starting", "idle")).toBe("starting");
  });

  test("resolveDisplayedCapturePhase falls back to legacy when idle", () => {
    expect(resolveDisplayedCapturePhase("idle", "idle", "idle")).toBe("idle");
  });

  test("audio controls disabled unless session active and capture capturing (req 4.1/4.2)", () => {
    const idleLegacy = resolveCapturePhaseForGate({
      sessionSubscribed: true,
      sessionPhase: "idle",
      sessionCapturePhase: "capturing",
      legacyPhase: "idle",
    });
    expect(idleLegacy).toBe("idle");
    expect(resolveCaptureAudioControlsDisabled("idle", idleLegacy)).toBe(true);

    const activeCapturing = resolveCapturePhaseForGate({
      sessionSubscribed: true,
      sessionPhase: "active",
      sessionCapturePhase: "capturing",
      legacyPhase: "idle",
    });
    expect(activeCapturing).toBe("capturing");
    expect(resolveCaptureAudioControlsDisabled("active", activeCapturing)).toBe(false);

    const activeNotCapturing = resolveCapturePhaseForGate({
      sessionSubscribed: true,
      sessionPhase: "active",
      sessionCapturePhase: "starting",
      legacyPhase: "idle",
    });
    expect(resolveCaptureAudioControlsDisabled("active", activeNotCapturing)).toBe(true);
  });
});
