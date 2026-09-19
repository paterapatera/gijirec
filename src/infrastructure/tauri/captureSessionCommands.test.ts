import { describe, expect, test } from "bun:test";
import type {
  CaptureSessionCapturePhase,
  CaptureSessionErrorCode,
  CaptureSessionInvokeError,
  CaptureSessionPhase,
  CaptureSessionState,
  CaptureSessionStateChanged,
} from "../../presentation/hooks/capture-session-types";
import { CAPTURE_SESSION_STATE_CHANGED_EVENT } from "../../presentation/hooks/capture-session-types";
import { getCaptureSessionState, startCaptureSession } from "./captureSessionCommands";
import { asInjectableInvokeFn } from "./injectableInvoke";

type InvokeCall = {
  command: string;
  args?: unknown;
};

function createMockInvoke<T>(response: T) {
  const calls: InvokeCall[] = [];
  const invokeFn = async (command: string, args?: unknown) => {
    calls.push({ command, args });
    return response;
  };
  return { invokeFn: asInjectableInvokeFn(invokeFn), calls };
}

const sampleState: CaptureSessionState = {
  session_phase: "idle",
  transition_busy: false,
  capture_phase: "idle",
  timestamp_ms: 1_700_000_000_000,
};

describe("captureSessionCommands", () => {
  test("contract type mirrors stay aligned with invoke payloads", () => {
    const sessionPhases: CaptureSessionPhase[] = ["idle", "starting", "active"];
    const capturePhases: CaptureSessionCapturePhase[] = [
      "idle",
      "starting",
      "capturing",
      "stopping",
      "error",
    ];
    const changed: CaptureSessionStateChanged = { state: sampleState };
    const errorCodes: CaptureSessionErrorCode[] = [
      "TRANSITION_BUSY",
      "CAPTURE_START_FAILED",
      "UNSUPPORTED_PLATFORM",
      "INTERNAL",
    ];
    const invokeError: CaptureSessionInvokeError = {
      code: "TRANSITION_BUSY",
      message_ja: "処理中",
      action_ja: "待機",
    };
    expect(sessionPhases).toHaveLength(3);
    expect(capturePhases).toHaveLength(5);
    expect(changed.state.session_phase).toBe("idle");
    expect(errorCodes).toContain(invokeError.code);
  });

  test("event constant matches contract", () => {
    expect(CAPTURE_SESSION_STATE_CHANGED_EVENT).toBe("capture-session://state-changed");
  });

  test("getCaptureSessionState invokes get_capture_session_state without args", async () => {
    const { invokeFn, calls } = createMockInvoke(sampleState);
    const actual = await getCaptureSessionState({ invokeFn });
    expect(actual).toEqual(sampleState);
    expect(calls).toEqual([{ command: "get_capture_session_state" }]);
  });

  test("startCaptureSession invokes start_capture_session without args", async () => {
    const activeState: CaptureSessionState = {
      ...sampleState,
      session_phase: "active",
      capture_phase: "capturing",
    };
    const { invokeFn, calls } = createMockInvoke(activeState);
    const actual = await startCaptureSession({ invokeFn });
    expect(actual).toEqual(activeState);
    expect(calls).toEqual([{ command: "start_capture_session" }]);
  });
});
