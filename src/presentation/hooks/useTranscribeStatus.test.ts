import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { setupTestDom } from "../../test-setup";
import type {
  ModelDownloadProgress,
  TranscribeEventListenFn,
  TranscribePhaseChanged,
  TranscribeUserError,
} from "./transcribe-status";
import {
  INITIAL_TRANSCRIBE_STATUS,
  MODEL_PROGRESS_EVENT,
  PHASE_CHANGED_EVENT,
  TRANSCRIBE_ERROR_EVENT,
} from "./transcribe-status";
import { useTranscribeStatus } from "./useTranscribeStatus";

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

  const listenFn: TranscribeEventListenFn = async (event, handler) => {
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

describe("useTranscribeStatus", () => {
  test("starts in idle state", () => {
    const { listenFn } = createMockListen();
    const { result } = renderHook(() => useTranscribeStatus({ listenFn }));

    expect(result.current).toEqual(INITIAL_TRANSCRIBE_STATUS);
  });

  test("syncs initial phase from invoke on mount", async () => {
    const { listenFn } = createMockListen();
    const invokeFn = async () =>
      ({
        phase: {
          phase: "ready",
          timestamp_ms: 42,
        },
        model_progress: null,
      }) satisfies {
        phase: TranscribePhaseChanged;
        model_progress: ModelDownloadProgress | null;
      };

    const { result } = renderHook(() => useTranscribeStatus({ listenFn, invokeFn }));

    await waitFor(() => {
      expect(result.current.phase).toBe("ready");
    });
    expect(result.current.timestampMs).toBe(42);
  });

  test("updates phase and timestamp on phase-changed event", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { result } = renderHook(() => useTranscribeStatus({ listenFn }));

    await waitFor(() => {
      expect(listeners.has(PHASE_CHANGED_EVENT)).toBe(true);
    });

    act(() => {
      emit(PHASE_CHANGED_EVENT, {
        phase: "loading_model",
        timestamp_ms: 1000,
      } satisfies TranscribePhaseChanged);
    });

    await waitFor(() => {
      expect(result.current.phase).toBe("loading_model");
    });
    expect(result.current.timestampMs).toBe(1000);
    expect(result.current.error).toBeNull();
  });

  test("promotes idle to loading_model when model progress arrives", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { result } = renderHook(() => useTranscribeStatus({ listenFn }));

    await waitFor(() => {
      expect(listeners.has(MODEL_PROGRESS_EVENT)).toBe(true);
    });

    act(() => {
      emit(MODEL_PROGRESS_EVENT, {
        bytes_downloaded: 1_000,
        bytes_total: 10_000,
        percent: 10,
        status: "downloading",
      } satisfies ModelDownloadProgress);
    });

    await waitFor(() => {
      expect(result.current.phase).toBe("loading_model");
    });
  });

  test("updates model progress on model-progress event", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { result } = renderHook(() => useTranscribeStatus({ listenFn }));

    await waitFor(() => {
      expect(listeners.has(MODEL_PROGRESS_EVENT)).toBe(true);
    });

    act(() => {
      emit(MODEL_PROGRESS_EVENT, {
        bytes_downloaded: 50_000,
        bytes_total: 100_000,
        percent: 50,
        status: "downloading",
      } satisfies ModelDownloadProgress);
    });

    await waitFor(() => {
      expect(result.current.modelProgress).toEqual({
        bytes_downloaded: 50_000,
        bytes_total: 100_000,
        percent: 50,
        status: "downloading",
      });
    });
  });

  test("updates error on error event", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { result } = renderHook(() => useTranscribeStatus({ listenFn }));

    await waitFor(() => {
      expect(listeners.has(TRANSCRIBE_ERROR_EVENT)).toBe(true);
    });

    const mockError: TranscribeUserError = {
      code: "MODEL_DOWNLOAD_FAILED",
      message_ja: "モデルのダウンロードに失敗しました",
      action_ja: "ネットワーク接続を確認して再起動してください",
      recoverable: true,
    };

    act(() => {
      emit(TRANSCRIBE_ERROR_EVENT, mockError);
    });

    await waitFor(() => {
      expect(result.current.phase).toBe("error");
    });
    expect(result.current.error).toEqual(mockError);
  });

  test("cleans up event listeners on unmount", async () => {
    const { listenFn, unlistenEvents } = createMockListen();
    const { unmount } = renderHook(() => useTranscribeStatus({ listenFn }));

    await waitFor(() => {
      expect(unlistenEvents.length).toBe(0);
    });

    unmount();

    expect(unlistenEvents).toContain(PHASE_CHANGED_EVENT);
    expect(unlistenEvents).toContain(MODEL_PROGRESS_EVENT);
    expect(unlistenEvents).toContain(TRANSCRIBE_ERROR_EVENT);
  });
});
