import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { cleanup, fireEvent, render } from "@testing-library/react";
import { setupTestDom } from "../../test-setup";
import type { UseCaptureSessionResult } from "../hooks/useCaptureSession";
import { CaptureSessionStartControl } from "./CaptureSessionStartControl";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

function createInjectedProps(
  overrides: Partial<UseCaptureSessionResult> = {},
): UseCaptureSessionResult {
  return {
    session_phase: "idle",
    transition_busy: false,
    capture_phase: "idle",
    timestamp_ms: 0,
    disabled: false,
    busy: false,
    startCaptureSession: async () => ({
      session_phase: "active",
      transition_busy: false,
      capture_phase: "capturing",
      timestamp_ms: 0,
    }),
    ...overrides,
  };
}

function renderToggle(overrides: Partial<UseCaptureSessionResult> = {}) {
  const injected = createInjectedProps(overrides);
  return render(
    <CaptureSessionStartControl
      session_phase={injected.session_phase}
      disabled={injected.disabled}
      busy={injected.busy}
      startCaptureSession={injected.startCaptureSession}
    />,
  );
}

describe("CaptureSessionStartControl (task 13.1 / req 2.1–2.3, 2.6)", () => {
  test("renders start affordance when session is idle (req 2.2)", () => {
    const { getByTestId, queryByTestId } = renderToggle({ session_phase: "idle" });
    expect(getByTestId("capture-session-toggle-icon-start")).toBeTruthy();
    expect(queryByTestId("capture-session-toggle-icon-stop")).toBeNull();
  });

  test("does not render stop affordance when session is active (req 2.2)", () => {
    const { queryByTestId } = renderToggle({ session_phase: "active" });
    expect(queryByTestId("capture-session-toggle-section")).toBeNull();
    expect(queryByTestId("capture-session-toggle-icon-stop")).toBeNull();
  });

  test("disables toggle and sets aria-busy while transition_busy (req 2.5/2.6)", () => {
    const { getByTestId } = renderToggle({
      session_phase: "starting",
      disabled: true,
      busy: true,
    });
    const button = getByTestId("capture-session-toggle");
    expect(button.hasAttribute("disabled")).toBe(true);
    expect(button.getAttribute("aria-busy")).toBe("true");
  });

  test("click requests start when idle", async () => {
    let called = false;
    const { getByTestId } = renderToggle({
      session_phase: "idle",
      startCaptureSession: async () => {
        called = true;
        return createInjectedProps({ session_phase: "starting" });
      },
    });
    fireEvent.click(getByTestId("capture-session-toggle"));
    expect(called).toBe(true);
  });

  test("does not invoke start when disabled", () => {
    let callCount = 0;
    const { getByTestId } = renderToggle({
      session_phase: "idle",
      disabled: true,
      busy: true,
      startCaptureSession: async () => {
        callCount += 1;
        return createInjectedProps();
      },
    });
    fireEvent.click(getByTestId("capture-session-toggle"));
    expect(callCount).toBe(0);
  });

  test("starting shows start icon during transition", () => {
    const { getByTestId } = renderToggle({ session_phase: "starting", disabled: true, busy: true });
    expect(getByTestId("capture-session-toggle-icon-start")).toBeTruthy();
  });

  test("starting exposes in-progress label for a11y (req 2.1)", () => {
    const { getByTestId } = renderToggle({ session_phase: "starting", disabled: true, busy: true });
    expect(getByTestId("capture-session-toggle").getAttribute("aria-label")).toBe(
      "キャプチャセッションを開始中",
    );
  });

  test("active session removes start control from DOM (req 2.2)", () => {
    const { queryByTestId } = renderToggle({ session_phase: "active" });
    expect(queryByTestId("capture-session-toggle-section")).toBeNull();
    expect(queryByTestId("capture-session-toggle")).toBeNull();
  });
});
