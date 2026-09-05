import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { App } from "./App";
import type {
  CaptureEventListenFn,
  CapturePhaseChanged,
  CaptureUserError,
} from "./hooks/capture-status";
import { ERROR_EVENT, PHASE_CHANGED_EVENT } from "./hooks/capture-status";
import { setupTestDom } from "./test-setup";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

type EventHandler = (event: { payload: unknown }) => void;

function createMockListen() {
  const listeners = new Map<string, EventHandler[]>();

  const listenFn: CaptureEventListenFn = async (event, handler) => {
    const handlers = listeners.get(event) ?? [];
    handlers.push(handler as EventHandler);
    listeners.set(event, handlers);
    return () => {
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

  return { listenFn, emit, listeners };
}

describe("App", () => {
  // E2E/UI Test 1: アプリ起動後 UI に capturing 表示 (req 3.1)
  // Simulates Tauri capture-phase-changed event after lifecycle wiring (7.1).
  test("shows capturing phase after phase-changed event", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { getByTestId } = render(<App listenFn={listenFn} />);

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
      expect(getByTestId("capture-phase").textContent).toBe("capturing");
    });
  });

  // E2E/UI Test 2: マイク権限拒否で action_ja 含有エラー表示 (req 5.4, 7.1)
  // UI must not expose technical error codes to the user.
  test("shows message_ja and prominent action_ja for permission denied", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { getByTestId, container } = render(<App listenFn={listenFn} />);

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
      expect(getByTestId("error-message").textContent).toBe(payload.message_ja);
    });

    const action = getByTestId("error-action");
    expect(action.textContent).toBe(payload.action_ja);
    expect(action.className).toContain("error-action");

    expect(container.textContent).not.toContain("MIC_PERMISSION_DENIED");
  });
});
