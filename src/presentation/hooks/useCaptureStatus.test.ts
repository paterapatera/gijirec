import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { asInjectableInvokeFn } from "../../infrastructure/tauri/injectableInvoke";
import { setupTestDom } from "../../test-setup";
import type { CaptureEventListenFn, CapturePhaseChanged, CaptureUserError } from "./capture-status";
import { ERROR_EVENT, INITIAL_CAPTURE_STATUS, PHASE_CHANGED_EVENT } from "./capture-status";
import { useCaptureStatus } from "./useCaptureStatus";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

type EventHandler = (event: { payload: unknown }) => void;

function createMockListen() {
  const listeners = new Map<string, EventHandler[]>();
  const unlistenEvents: string[] = [];

  const listenFn: CaptureEventListenFn = async (event, handler) => {
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

describe("useCaptureStatus", () => {
  test("starts in idle state", () => {
    const { listenFn } = createMockListen();
    const { result } = renderHook(() => useCaptureStatus({ listenFn }));

    expect(result.current).toEqual(INITIAL_CAPTURE_STATUS);
  });

  test("syncs current phase from invoke on mount", async () => {
    const { listenFn } = createMockListen();
    const invokeFn = asInjectableInvokeFn(
      async () =>
        ({
          phase: "capturing",
          timestamp_ms: 9_876_543_210,
        }) as CapturePhaseChanged,
    );

    const { result } = renderHook(() => useCaptureStatus({ listenFn, invokeFn }));

    await waitFor(() => {
      expect(result.current.phase).toBe("capturing");
    });
    expect(result.current.timestampMs).toBe(9_876_543_210);
  });

  test("updates phase when phase-changed is emitted", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { result } = renderHook(() => useCaptureStatus({ listenFn }));

    await waitFor(() => {
      expect(listeners.has(PHASE_CHANGED_EVENT)).toBe(true);
    });

    const payload: CapturePhaseChanged = {
      phase: "capturing",
      timestamp_ms: 1_234_567_890,
    };
    act(() => {
      emit(PHASE_CHANGED_EVENT, payload);
    });

    await waitFor(() => {
      expect(result.current.phase).toBe("capturing");
    });
    expect(result.current.timestampMs).toBe(1_234_567_890);
    expect(result.current.error).toBeNull();
  });

  test("stores message_ja and action_ja from error event", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { result } = renderHook(() => useCaptureStatus({ listenFn }));

    await waitFor(() => {
      expect(listeners.has(ERROR_EVENT)).toBe(true);
    });

    const payload: CaptureUserError = {
      code: "MIC_PERMISSION_DENIED",
      message_ja: "マイクへのアクセスが拒否されました",
      action_ja: "設定 → プライバシー → マイクで gijirec を許可してください",
      recoverable: true,
    };
    act(() => {
      emit(ERROR_EVENT, payload);
    });

    await waitFor(() => {
      expect(result.current.error).not.toBeNull();
    });
    expect(result.current.error?.message_ja).toBe(payload.message_ja);
    expect(result.current.error?.action_ja).toBe(payload.action_ja);
    expect(result.current.error?.action_ja.length).toBeGreaterThan(0);
    expect(result.current.phase).toBe("error");
  });

  test("unmount unlistens from both events", async () => {
    const { listenFn, unlistenEvents, listeners } = createMockListen();
    const { unmount } = renderHook(() => useCaptureStatus({ listenFn }));

    await waitFor(() => {
      expect(listeners.has(PHASE_CHANGED_EVENT)).toBe(true);
      expect(listeners.has(ERROR_EVENT)).toBe(true);
    });

    unmount();

    await waitFor(() => {
      expect(unlistenEvents).toContain(PHASE_CHANGED_EVENT);
      expect(unlistenEvents).toContain(ERROR_EVENT);
    });
  });
});
