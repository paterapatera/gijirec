/**
 * E2E/UI smoke (design Testing Strategy — E2E/UI Tests, task 10.3):
 * capturing / non-capturing display diff, dBFS label (req 2.3), inactive meter (req 2.4),
 * horizontal layout + fixed-width meter (req 4.5, 5.2).
 */
import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { act, cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { setupTestDom } from "../../test-setup";
import type {
  CaptureAudioControls,
  CaptureAudioControlsState,
  IngestLevelSnapshot,
} from "../hooks/capture-audio-controls-types";
import { DEFAULT_INGEST_GAIN } from "../hooks/capture-audio-controls-types";
import { CaptureAudioControlsRow } from "./CaptureAudioControlsRow";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

const defaultControls: CaptureAudioControls = {
  mic_ingest_enabled: true,
  manual_ingest_gain: DEFAULT_INGEST_GAIN,
  gain_user_adjusted: false,
};

const activeLevel: IngestLevelSnapshot = {
  level_dbfs: -18.2,
  timestamp_ms: 1_700_000_000_000,
};

type UiSnapshot = {
  meterText: string;
  meterAriaLabel: string | null;
  micDisabled: boolean;
  sliderDisabled: boolean;
  micChecked: boolean;
  gainValue: string;
  sectionClass: string;
  meterClass: string;
};

function readUiSnapshot(container: ReturnType<typeof render>): UiSnapshot {
  const meter = container.getByTestId("ingest-level-meter");
  const micSwitch = container.getByTestId("mic-ingest-switch");
  const slider = container.getByTestId("ingest-gain-slider") as HTMLInputElement;
  const section = container.getByTestId("capture-audio-controls-row");

  return {
    meterText: meter.textContent ?? "",
    meterAriaLabel: meter.getAttribute("aria-label"),
    micDisabled: micSwitch.hasAttribute("disabled"),
    sliderDisabled: slider.disabled,
    micChecked: micSwitch.getAttribute("data-state") === "checked",
    gainValue: slider.value,
    sectionClass: section.className,
    meterClass: meter.className,
  };
}

function createMockListen() {
  const listeners = new Map<string, ((event: { payload: unknown }) => void)[]>();
  const listenFn = async (event: string, handler: (event: { payload: unknown }) => void) => {
    const handlers = listeners.get(event) ?? [];
    handlers.push(handler);
    listeners.set(event, handlers);
    return () => {
      const list = listeners.get(event) ?? [];
      const index = list.indexOf(handler);
      if (index >= 0) {
        list.splice(index, 1);
      }
    };
  };
  return { listenFn };
}

function createConnectedInvoke(
  backendState: CaptureAudioControlsState,
  phase: "idle" | "capturing",
) {
  const calls: { command: string; args?: unknown }[] = [];
  let state = { ...backendState, controls: { ...backendState.controls } };

  const invokeFn = async (command: string, args?: unknown) => {
    calls.push({ command, args });
    switch (command) {
      case "get_capture_phase":
        return { phase, timestamp_ms: 0 };
      case "get_capture_audio_controls":
        return state;
      case "set_capture_audio_controls":
        state = {
          ...state,
          controls: { ...state.controls, ...(args as Partial<CaptureAudioControls>) },
        };
        return state;
      default:
        throw new Error(`unexpected invoke: ${command}`);
    }
  };

  return { invokeFn, calls };
}

describe("CaptureAudioControlsRow E2E/UI", () => {
  // E2E/UI 1 (req 2.3, 2.4, 3.1, 3.5): capturing vs non-capturing UI snapshot diff
  test("UI snapshot differs between capturing and non-capturing states", () => {
    const capturing = readUiSnapshot(
      render(
        <CaptureAudioControlsRow
          controls={defaultControls}
          ingest_level={activeLevel}
          disabled={false}
        />,
      ),
    );
    cleanup();

    const nonCapturing = readUiSnapshot(
      render(
        <CaptureAudioControlsRow
          controls={defaultControls}
          ingest_level={activeLevel}
          disabled={true}
        />,
      ),
    );

    expect(capturing).toMatchInlineSnapshot(`
      {
        "gainValue": "1.25",
        "meterAriaLabel": "ingest 直前レベル",
        "meterClass": "capture-audio-meter",
        "meterText": "−18.2 dBFS",
        "micChecked": true,
        "micDisabled": false,
        "sectionClass": "capture-audio-controls-row",
        "sliderDisabled": false,
      }
    `);

    expect(nonCapturing).toMatchInlineSnapshot(`
      {
        "gainValue": "1.25",
        "meterAriaLabel": "レベルメーター非活性",
        "meterClass": "capture-audio-meter",
        "meterText": "—",
        "micChecked": true,
        "micDisabled": true,
        "sectionClass": "capture-audio-controls-row",
        "sliderDisabled": true,
      }
    `);

    expect(capturing.meterText).not.toBe(nonCapturing.meterText);
    expect(capturing.micDisabled).toBe(false);
    expect(nonCapturing.micDisabled).toBe(true);
  });

  // E2E/UI 2 (req 2.3): dBFS unit label is visible and identifiable
  test("shows dBFS suffix on active meter label", () => {
    const { getByTestId } = render(
      <CaptureAudioControlsRow
        controls={defaultControls}
        ingest_level={{ level_dbfs: -17.0, timestamp_ms: 0 }}
        disabled={false}
      />,
    );

    const meter = getByTestId("ingest-level-meter");
    expect(meter.textContent).toBe("−17.0 dBFS");
    expect(meter.textContent).toContain("dBFS");
  });

  // E2E/UI 3 (req 2.4): no misleading fixed level when ingest unavailable
  test("shows inactive meter when ingest_level is null even if not disabled", () => {
    const snapshot = readUiSnapshot(
      render(
        <CaptureAudioControlsRow
          controls={defaultControls}
          ingest_level={null}
          disabled={false}
        />,
      ),
    );

    expect(snapshot.meterText).toBe("—");
    expect(snapshot.meterAriaLabel).toBe("レベルメーター非活性");
  });

  // E2E/UI 4 (req 4.5, 5.2): layout classes for horizontal row and fixed-width meter
  test("uses stable layout classes for meter and control row", () => {
    const { getByTestId } = render(
      <CaptureAudioControlsRow
        controls={defaultControls}
        ingest_level={activeLevel}
        disabled={false}
      />,
    );

    expect(getByTestId("capture-audio-controls-row").classList.contains("capture-audio-controls-row")).toBe(
      true,
    );
    expect(getByTestId("ingest-level-meter").classList.contains("capture-audio-meter")).toBe(true);
  });

  // E2E/UI 5 (req 2.3, 2.4): connected mode smoke — phase drives snapshot
  test("connected mode snapshot matches capturing phase with ingest level", async () => {
    const { listenFn } = createMockListen();
    const { invokeFn } = createConnectedInvoke(
      { controls: defaultControls, ingest_level: activeLevel },
      "capturing",
    );

    const view = render(
      <CaptureAudioControlsRow invokeFn={invokeFn} listenFn={listenFn} capturePhase="capturing" />,
    );

    await waitFor(() => {
      expect(view.getByTestId("ingest-level-meter").textContent).toBe("−18.2 dBFS");
    });

    expect(readUiSnapshot(view)).toMatchObject({
      meterText: "−18.2 dBFS",
      micDisabled: false,
      sliderDisabled: false,
    });
  });

  test("connected mode snapshot matches idle phase with inactive meter", async () => {
    const { listenFn } = createMockListen();
    const { invokeFn } = createConnectedInvoke(
      { controls: defaultControls, ingest_level: null },
      "idle",
    );

    const view = render(
      <CaptureAudioControlsRow invokeFn={invokeFn} listenFn={listenFn} capturePhase="idle" />,
    );

    await waitFor(() => {
      expect(view.getByTestId("mic-ingest-switch").hasAttribute("disabled")).toBe(true);
    });

    expect(readUiSnapshot(view)).toMatchObject({
      meterText: "—",
      meterAriaLabel: "レベルメーター非活性",
      micDisabled: true,
      sliderDisabled: true,
    });
  });

  // E2E/UI 6 (req 1.1, 2.3): capturing smoke — toggle and slider invoke backend
  test("capturing smoke invokes set_capture_audio_controls on toggle and gain change", async () => {
    const { listenFn } = createMockListen();
    const { invokeFn, calls } = createConnectedInvoke(
      { controls: defaultControls, ingest_level: activeLevel },
      "capturing",
    );

    const { getByTestId } = render(
      <CaptureAudioControlsRow invokeFn={invokeFn} listenFn={listenFn} capturePhase="capturing" />,
    );

    await waitFor(() => {
      expect(getByTestId("mic-ingest-switch").hasAttribute("disabled")).toBe(false);
    });

    await act(async () => {
      fireEvent.click(getByTestId("mic-ingest-switch"));
    });

    const slider = getByTestId("ingest-gain-slider") as HTMLInputElement;
    const setNativeValue = Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      "value",
    )?.set;
    await act(async () => {
      setNativeValue?.call(slider, "2.0");
      fireEvent.input(slider, { target: { value: "2.0" } });
    });

    await waitFor(() => {
      expect(calls.filter((c) => c.command === "set_capture_audio_controls").length).toBe(2);
    });
    expect(calls).toContainEqual({
      command: "set_capture_audio_controls",
      args: { mic_ingest_enabled: false },
    });
    expect(calls).toContainEqual({
      command: "set_capture_audio_controls",
      args: { manual_ingest_gain: 2.0 },
    });
  });
});
