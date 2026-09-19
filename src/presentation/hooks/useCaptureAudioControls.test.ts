import { afterEach, beforeAll, describe, expect, mock, test } from "bun:test";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { asInjectableInvokeFn } from "../../infrastructure/tauri/injectableInvoke";
import { setupTestDom } from "../../test-setup";
import type {
  CaptureAudioControlsChanged,
  CaptureAudioControlsEventListenFn,
  CaptureAudioControlsState,
  IngestLevelChanged,
} from "./capture-audio-controls-types";
import {
  CONTROLS_CHANGED_EVENT,
  DEFAULT_INGEST_GAIN,
  INGEST_LEVEL_EVENT,
  INITIAL_CAPTURE_AUDIO_CONTROLS_HOOK_STATE,
} from "./capture-audio-controls-types";
import { CAPTURE_SESSION_STATE_CHANGED_EVENT } from "./capture-session-types";
import {
  resolveCaptureAudioControlsDisabled,
  useCaptureAudioControls,
} from "./useCaptureAudioControls";

type TauriEventHandler = (event: { payload: unknown }) => void;

mock.module("@tauri-apps/api/event", () => ({
  listen: async (event: string, handler: TauriEventHandler) => {
    const handlers = tauriEventListeners.get(event) ?? [];
    handlers.push(handler);
    tauriEventListeners.set(event, handlers);
    return () => {
      const list = tauriEventListeners.get(event) ?? [];
      const index = list.indexOf(handler);
      if (index >= 0) {
        list.splice(index, 1);
      }
    };
  },
}));

const tauriEventListeners = new Map<string, TauriEventHandler[]>();

function installTauriRuntimeStub(): void {
  Object.assign(window, {
    __TAURI_INTERNALS__: {
      transformCallback: (callback: () => void) => callback,
    },
  });
}

function removeTauriRuntimeStub(): void {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
}

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  tauriEventListeners.clear();
  removeTauriRuntimeStub();
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

  const listenFn: CaptureAudioControlsEventListenFn = async (event, handler) => {
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

const idlePhase = { phase: "idle" as const, timestamp_ms: 0 };
const capturingPhase = { phase: "capturing" as const, timestamp_ms: 1 };

const initialBackendState: CaptureAudioControlsState = {
  controls: {
    mic_ingest_enabled: true,
    manual_ingest_gain: DEFAULT_INGEST_GAIN,
    gain_user_adjusted: false,
  },
  ingest_level: null,
};

const stateWithLevel: CaptureAudioControlsState = {
  controls: {
    mic_ingest_enabled: false,
    manual_ingest_gain: 2.0,
    gain_user_adjusted: true,
  },
  ingest_level: {
    level_dbfs: -18.2,
    timestamp_ms: 1_700_000_000_000,
  },
};

function defaultInvokeHandlers(controlsState: CaptureAudioControlsState = initialBackendState) {
  return {
    get_capture_phase: () => idlePhase,
    get_capture_session_state: () => ({
      session_phase: "idle",
      transition_busy: false,
      capture_phase: "idle",
      timestamp_ms: 0,
    }),
    get_capture_audio_controls: () => controlsState,
  };
}

describe("useCaptureAudioControls", () => {
  test("starts with contract defaults and disabled when not capturing", async () => {
    const { listenFn } = createMockListen();
    const { invokeFn } = createMockInvoke(defaultInvokeHandlers());

    const { result } = renderHook(() =>
      useCaptureAudioControls({ listenFn, invokeFn, capturePhase: "idle" }),
    );

    await waitFor(() => {
      expect(result.current).toEqual(INITIAL_CAPTURE_AUDIO_CONTROLS_HOOK_STATE);
    });
  });

  test("syncs controls from get_capture_audio_controls on mount", async () => {
    const { listenFn } = createMockListen();
    const { invokeFn } = createMockInvoke(defaultInvokeHandlers(stateWithLevel));

    const { result } = renderHook(() =>
      useCaptureAudioControls({
        listenFn,
        invokeFn,
        capturePhase: "capturing",
        sessionPhase: "active",
      }),
    );

    await waitFor(() => {
      expect(result.current.controls.manual_ingest_gain).toBe(2.0);
    });
    expect(result.current.controls.mic_ingest_enabled).toBe(false);
    expect(result.current.ingest_level?.level_dbfs).toBe(-18.2);
    expect(result.current.disabled).toBe(false);
  });

  test("disabled unless session is active and capture is capturing", async () => {
    const { listenFn } = createMockListen();
    const { invokeFn } = createMockInvoke(defaultInvokeHandlers());

    const { result, rerender } = renderHook(
      ({
        sessionPhase,
        capturePhase,
      }: {
        sessionPhase: "idle" | "active";
        capturePhase: "idle" | "capturing";
      }) =>
        useCaptureAudioControls({
          listenFn,
          invokeFn,
          sessionPhase,
          capturePhase,
        }),
      { initialProps: { sessionPhase: "idle" as const, capturePhase: "idle" as const } },
    );

    expect(result.current.disabled).toBe(true);

    rerender({ sessionPhase: "active", capturePhase: "idle" });
    expect(result.current.disabled).toBe(true);

    rerender({ sessionPhase: "idle", capturePhase: "capturing" });
    expect(result.current.disabled).toBe(true);

    rerender({ sessionPhase: "active", capturePhase: "capturing" });

    await waitFor(() => {
      expect(result.current.disabled).toBe(false);
    });
  });

  test("derives session phase without inject listenFn when Tauri runtime is present", async () => {
    installTauriRuntimeStub();
    const { invokeFn } = createMockInvoke({
      ...defaultInvokeHandlers(),
      get_capture_phase: () => capturingPhase,
      get_capture_session_state: () => ({
        session_phase: "active",
        transition_busy: false,
        capture_phase: "capturing",
        timestamp_ms: 0,
      }),
    });

    const { result } = renderHook(() =>
      useCaptureAudioControls({ invokeFn, capturePhase: "capturing" }),
    );

    await waitFor(() => {
      expect(result.current.disabled).toBe(false);
    });
  });

  test("enables controls when session state-changed reports capturing while legacy phase stays idle", async () => {
    const { listenFn, emit } = createMockListen();
    const { invokeFn } = createMockInvoke(defaultInvokeHandlers());

    const { result } = renderHook(() => useCaptureAudioControls({ listenFn, invokeFn }));

    await waitFor(() => {
      expect(result.current.disabled).toBe(true);
    });

    act(() => {
      emit(CAPTURE_SESSION_STATE_CHANGED_EVENT, {
        state: {
          session_phase: "active",
          transition_busy: false,
          capture_phase: "capturing",
          timestamp_ms: 1,
        },
      });
    });

    await waitFor(() => {
      expect(result.current.disabled).toBe(false);
    });
  });

  test("derives phases from hooks when overrides are omitted", async () => {
    const { listenFn } = createMockListen();
    const { invokeFn } = createMockInvoke({
      ...defaultInvokeHandlers(),
      get_capture_phase: () => capturingPhase,
      get_capture_session_state: () => ({
        session_phase: "active",
        transition_busy: false,
        capture_phase: "capturing",
        timestamp_ms: 0,
      }),
    });

    const { result } = renderHook(() => useCaptureAudioControls({ listenFn, invokeFn }));

    await waitFor(() => {
      expect(result.current.disabled).toBe(false);
    });
  });

  test("updates controls when controls-changed is emitted", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { invokeFn } = createMockInvoke(defaultInvokeHandlers());

    const { result } = renderHook(() =>
      useCaptureAudioControls({
        listenFn,
        invokeFn,
        capturePhase: "capturing",
        sessionPhase: "active",
      }),
    );

    await waitFor(() => {
      expect(listeners.has(CONTROLS_CHANGED_EVENT)).toBe(true);
    });

    const payload: CaptureAudioControlsChanged = {
      controls: {
        mic_ingest_enabled: false,
        manual_ingest_gain: 3.0,
        gain_user_adjusted: true,
      },
      timestamp_ms: 42,
    };
    act(() => {
      emit(CONTROLS_CHANGED_EVENT, payload);
    });

    await waitFor(() => {
      expect(result.current.controls.mic_ingest_enabled).toBe(false);
    });
    expect(result.current.controls.manual_ingest_gain).toBe(3.0);
  });

  test("updates ingest_level when ingest-level is emitted", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { invokeFn } = createMockInvoke(defaultInvokeHandlers());

    const { result } = renderHook(() =>
      useCaptureAudioControls({
        listenFn,
        invokeFn,
        capturePhase: "capturing",
        sessionPhase: "active",
      }),
    );

    await waitFor(() => {
      expect(listeners.has(INGEST_LEVEL_EVENT)).toBe(true);
    });

    const payload: IngestLevelChanged = {
      level_dbfs: -17.8,
      timestamp_ms: 99,
    };
    act(() => {
      emit(INGEST_LEVEL_EVENT, payload);
    });

    await waitFor(() => {
      expect(result.current.ingest_level?.level_dbfs).toBe(-17.8);
    });
    expect(result.current.ingest_level?.timestamp_ms).toBe(99);
  });

  test("unmount unlistens from both events", async () => {
    const { listenFn, unlistenEvents, listeners } = createMockListen();
    const { invokeFn } = createMockInvoke(defaultInvokeHandlers());

    const { unmount } = renderHook(() =>
      useCaptureAudioControls({
        listenFn,
        invokeFn,
        capturePhase: "capturing",
        sessionPhase: "active",
      }),
    );

    await waitFor(() => {
      expect(listeners.has(CONTROLS_CHANGED_EVENT)).toBe(true);
      expect(listeners.has(INGEST_LEVEL_EVENT)).toBe(true);
    });

    unmount();

    await waitFor(() => {
      expect(unlistenEvents).toContain(CONTROLS_CHANGED_EVENT);
      expect(unlistenEvents).toContain(INGEST_LEVEL_EVENT);
    });
  });

  test("resolveCaptureAudioControlsDisabled matches session and capture gate", () => {
    expect(resolveCaptureAudioControlsDisabled("idle", "idle")).toBe(true);
    expect(resolveCaptureAudioControlsDisabled("idle", "capturing")).toBe(true);
    expect(resolveCaptureAudioControlsDisabled("active", "idle")).toBe(true);
    expect(resolveCaptureAudioControlsDisabled("active", "capturing")).toBe(false);
  });
});
