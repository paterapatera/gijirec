import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { asInjectableInvokeFn } from "../../infrastructure/tauri/injectableInvoke";
import { setupTestDom } from "../../test-setup";
import type {
  CaptureSessionEventListenFn,
  CaptureSessionState,
  CaptureSessionStateChanged,
} from "./capture-session-types";
import {
  CAPTURE_SESSION_STATE_CHANGED_EVENT,
  INITIAL_CAPTURE_SESSION_HOOK_STATE,
} from "./capture-session-types";
import { useCaptureSession } from "./useCaptureSession";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

type EventHandler = (event: { payload: unknown }) => void;

type InvokeCall = {
  command: string;
  args?: unknown;
};

function createMockListen() {
  const listeners = new Map<string, EventHandler[]>();
  const unlistenEvents: string[] = [];

  const listenFn: CaptureSessionEventListenFn = async (event, handler) => {
    const handlers = listeners.get(event) ?? [];
    handlers.push(handler as EventHandler);
    listeners.set(event, handlers);
    return () => {
      unlistenEvents.push(event);
      const list = listeners.get(event) ?? [];
      const index = list.indexOf(handler as EventHandler);
      if (index >= 0) {
        list.splice(index, 1);
      }
    };
  };

  const emit = (event: string, payload: unknown) => {
    for (const handler of listeners.get(event) ?? []) {
      handler({ payload });
    }
  };

  return { listenFn, emit, unlistenEvents, listeners };
}

function createMockInvoke(handlers: Record<string, (args?: unknown) => unknown>) {
  const calls: InvokeCall[] = [];
  const invokeFn = async (command: string, args?: unknown) => {
    calls.push({ command, args });
    const handler = handlers[command];
    if (handler === undefined) {
      throw new Error(`unexpected invoke: ${command}`);
    }
    return handler(args);
  };
  return { invokeFn: asInjectableInvokeFn(invokeFn), calls };
}

const idleState: CaptureSessionState = {
  session_phase: "idle",
  transition_busy: false,
  capture_phase: "idle",
  timestamp_ms: 100,
};

const activeBusyState: CaptureSessionState = {
  session_phase: "starting",
  transition_busy: true,
  capture_phase: "starting",
  timestamp_ms: 200,
};

function defaultInvokeHandlers(sessionState: CaptureSessionState = idleState) {
  return {
    get_capture_session_state: () => sessionState,
    start_capture_session: () => ({
      ...sessionState,
      session_phase: "active" as const,
      transition_busy: false,
      capture_phase: "capturing" as const,
      timestamp_ms: sessionState.timestamp_ms + 1,
    }),
  };
}

describe("useCaptureSession", () => {
  test("starts with contract defaults before mount sync", () => {
    const { listenFn } = createMockListen();
    const { invokeFn } = createMockInvoke(defaultInvokeHandlers());

    const { result } = renderHook(() => useCaptureSession({ listenFn, invokeFn }));

    expect(result.current.session_phase).toBe("idle");
    expect(result.current.disabled).toBe(false);
    expect(result.current.busy).toBe(false);
  });

  test("syncs session state from get_capture_session_state on mount", async () => {
    const { listenFn } = createMockListen();
    const backendState: CaptureSessionState = {
      session_phase: "active",
      transition_busy: false,
      capture_phase: "capturing",
      timestamp_ms: 9_876_543_210,
    };
    const { invokeFn, calls } = createMockInvoke(defaultInvokeHandlers(backendState));

    const { result } = renderHook(() => useCaptureSession({ listenFn, invokeFn }));

    await waitFor(() => {
      expect(result.current.session_phase).toBe("active");
    });
    expect(calls).toContainEqual({ command: "get_capture_session_state" });
    expect(result.current.capture_phase).toBe("capturing");
    expect(result.current.timestamp_ms).toBe(9_876_543_210);
    expect(result.current.disabled).toBe(false);
  });

  test("updates state when capture-session state-changed is emitted", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { invokeFn } = createMockInvoke(defaultInvokeHandlers());

    const { result } = renderHook(() => useCaptureSession({ listenFn, invokeFn }));

    await waitFor(() => {
      expect(listeners.has(CAPTURE_SESSION_STATE_CHANGED_EVENT)).toBe(true);
    });

    const payload: CaptureSessionStateChanged = {
      state: {
        session_phase: "active",
        transition_busy: false,
        capture_phase: "capturing",
        timestamp_ms: 1_234_567_890,
      },
    };
    act(() => {
      emit(CAPTURE_SESSION_STATE_CHANGED_EVENT, payload);
    });

    await waitFor(() => {
      expect(result.current.session_phase).toBe("active");
    });
    expect(result.current.capture_phase).toBe("capturing");
    expect(result.current.timestamp_ms).toBe(1_234_567_890);
  });

  test("sets disabled and busy when transition_busy is true", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { invokeFn } = createMockInvoke(defaultInvokeHandlers(activeBusyState));

    const { result } = renderHook(() => useCaptureSession({ listenFn, invokeFn }));

    await waitFor(() => {
      expect(listeners.has(CAPTURE_SESSION_STATE_CHANGED_EVENT)).toBe(true);
    });

    await waitFor(() => {
      expect(result.current.transition_busy).toBe(true);
    });
    expect(result.current.disabled).toBe(true);
    expect(result.current.busy).toBe(true);

    const cleared: CaptureSessionStateChanged = {
      state: {
        ...activeBusyState,
        session_phase: "active",
        transition_busy: false,
        capture_phase: "capturing",
      },
    };
    act(() => {
      emit(CAPTURE_SESSION_STATE_CHANGED_EVENT, cleared);
    });

    await waitFor(() => {
      expect(result.current.transition_busy).toBe(false);
    });
    expect(result.current.disabled).toBe(false);
    expect(result.current.busy).toBe(false);
  });

  test("startCaptureSession invokes command and mirrors returned state", async () => {
    const { listenFn } = createMockListen();
    const { invokeFn, calls } = createMockInvoke(defaultInvokeHandlers());

    const { result } = renderHook(() => useCaptureSession({ listenFn, invokeFn }));

    await waitFor(() => {
      expect(result.current.session_phase).toBe("idle");
    });

    await act(async () => {
      await result.current.startCaptureSession();
    });

    expect(calls).toContainEqual({ command: "start_capture_session" });
    expect(result.current.session_phase).toBe("active");
    expect(result.current.capture_phase).toBe("capturing");
  });

  test("startCaptureSession propagates invoke errors", async () => {
    const { listenFn } = createMockListen();
    const invokeFn = asInjectableInvokeFn(async (command: string) => {
      if (command === "get_capture_session_state") {
        return idleState;
      }
      throw Object.assign(new Error("TRANSITION_BUSY"), {
        code: "TRANSITION_BUSY",
        message_ja: "処理中",
        action_ja: "待って",
      });
    });

    const { result } = renderHook(() => useCaptureSession({ listenFn, invokeFn }));

    await waitFor(() => {
      expect(result.current.session_phase).toBe("idle");
    });

    let thrown: unknown;
    await act(async () => {
      try {
        await result.current.startCaptureSession();
      } catch (error) {
        thrown = error;
      }
    });
    expect(thrown).toMatchObject({ code: "TRANSITION_BUSY" });
  });

  test("initial hook state matches INITIAL_CAPTURE_SESSION_HOOK_STATE fields", () => {
    const { listenFn } = createMockListen();
    const { invokeFn } = createMockInvoke({});
    const { result } = renderHook(() => useCaptureSession({ listenFn, invokeFn }));

    const { startCaptureSession, ...stateOnly } = result.current;
    expect(stateOnly).toEqual(INITIAL_CAPTURE_SESSION_HOOK_STATE);
    expect(typeof startCaptureSession).toBe("function");
  });
});
