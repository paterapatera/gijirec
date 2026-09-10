import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { act, cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { setupTestDom } from "../../test-setup";
import type {
  CaptureAudioControls,
  CaptureAudioControlsState,
  IngestLevelSnapshot,
} from "../hooks/capture-audio-controls-types";
import {
  DEFAULT_INGEST_GAIN,
  MAX_INGEST_GAIN,
  MIN_INGEST_GAIN,
} from "../hooks/capture-audio-controls-types";
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

type RenderOverrides = {
  controls?: CaptureAudioControls;
  ingest_level?: IngestLevelSnapshot | null;
  disabled?: boolean;
  capturePhase?: "idle" | "capturing";
  invokeFn?: (command: string, args?: unknown) => Promise<unknown>;
  listenFn?: (event: string, handler: (event: { payload: unknown }) => void) => Promise<() => void>;
};

function renderRow(overrides: RenderOverrides = {}) {
  const props: Record<string, unknown> = {
    controls: overrides.controls ?? defaultControls,
    ingest_level: overrides.ingest_level ?? null,
    disabled: overrides.disabled ?? false,
  };
  if (overrides.capturePhase !== undefined) {
    props.capturePhase = overrides.capturePhase;
  }
  if (overrides.invokeFn !== undefined) {
    props.invokeFn = overrides.invokeFn;
  }
  if (overrides.listenFn !== undefined) {
    props.listenFn = overrides.listenFn;
  }
  return render(<CaptureAudioControlsRow {...props} />);
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
  return { listenFn, listeners };
}

function createConnectedInvoke(
  backendState: CaptureAudioControlsState,
  phase: "idle" | "capturing" = "capturing",
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

  return { invokeFn, calls, getState: () => state };
}

describe("CaptureAudioControlsRow", () => {
  test("renders mic switch, dBFS meter, and gain slider", () => {
    const { getByTestId } = renderRow({
      ingest_level: activeLevel,
    });

    expect(getByTestId("mic-ingest-switch")).toBeTruthy();
    expect(getByTestId("ingest-level-meter")).toBeTruthy();
    expect(getByTestId("ingest-gain-slider")).toBeTruthy();
  });

  test("shows formatted dBFS label when capturing with ingest_level", () => {
    const { getByTestId } = renderRow({
      ingest_level: activeLevel,
      disabled: false,
    });

    expect(getByTestId("ingest-level-meter").textContent).toBe("−18.2 dBFS");
  });

  test("shows em dash meter when disabled", () => {
    const { getByTestId } = renderRow({
      ingest_level: activeLevel,
      disabled: true,
    });

    expect(getByTestId("ingest-level-meter").textContent).toBe("—");
  });

  test("shows inactive meter when ingest_level is null", () => {
    const { getByTestId } = renderRow({
      ingest_level: null,
      disabled: false,
    });

    expect(getByTestId("ingest-level-meter").textContent).toBe("—");
  });

  test("disables all controls when disabled is true", () => {
    const { getByTestId } = renderRow({ disabled: true });

    const switchEl = getByTestId("mic-ingest-switch");
    const slider = getByTestId("ingest-gain-slider") as HTMLInputElement;

    expect(switchEl.hasAttribute("disabled")).toBe(true);
    expect(slider.disabled).toBe(true);
  });

  test("gain slider has contract min, max, and step", () => {
    const { getByTestId } = renderRow();

    const slider = getByTestId("ingest-gain-slider") as HTMLInputElement;
    expect(slider.min).toBe(String(MIN_INGEST_GAIN));
    expect(slider.max).toBe(String(MAX_INGEST_GAIN));
    expect(slider.step).toBe("0.05");
    expect(slider.value).toBe(String(DEFAULT_INGEST_GAIN));
  });

  test("meter has fixed-width class to prevent layout shift", () => {
    const { getByTestId } = renderRow({ ingest_level: activeLevel });

    expect(getByTestId("ingest-level-meter").classList.contains("capture-audio-meter")).toBe(true);
  });

  test("shows Japanese gain min hint via aria-live when at lower limit", () => {
    const { getByTestId } = renderRow({
      controls: { ...defaultControls, manual_ingest_gain: MIN_INGEST_GAIN },
      ingest_level: activeLevel,
    });

    const hint = getByTestId("gain-limit-hint");
    expect(hint.getAttribute("aria-live")).toBe("polite");
    expect(hint.textContent).toContain("下限");
  });

  test("shows Japanese gain max hint via aria-live when at upper limit", () => {
    const { getByTestId } = renderRow({
      controls: { ...defaultControls, manual_ingest_gain: MAX_INGEST_GAIN },
      ingest_level: activeLevel,
    });

    const hint = getByTestId("gain-limit-hint");
    expect(hint.getAttribute("aria-live")).toBe("polite");
    expect(hint.textContent).toContain("上限");
  });

  test("calls set_capture_audio_controls when mic switch is toggled", async () => {
    const calls: { command: string; args?: unknown }[] = [];
    const invokeFn = async (command: string, args?: unknown) => {
      calls.push({ command, args });
      if (command === "set_capture_audio_controls") {
        return {
          controls: { ...defaultControls, ...(args as Partial<CaptureAudioControls>) },
          ingest_level: null,
        };
      }
      return undefined;
    };

    const { getByTestId } = renderRow({ invokeFn });

    await act(async () => {
      fireEvent.click(getByTestId("mic-ingest-switch"));
    });

    await waitFor(() => {
      expect(calls.some((c) => c.command === "set_capture_audio_controls")).toBe(true);
    });
    expect(calls).toContainEqual({
      command: "set_capture_audio_controls",
      args: { mic_ingest_enabled: false },
    });
  });

  test("calls set_capture_audio_controls when gain slider changes", async () => {
    const calls: { command: string; args?: unknown }[] = [];
    const invokeFn = async (command: string, args?: unknown) => {
      calls.push({ command, args });
      if (command === "set_capture_audio_controls") {
        return {
          controls: { ...defaultControls, ...(args as Partial<CaptureAudioControls>) },
          ingest_level: null,
        };
      }
      return undefined;
    };

    const { getByTestId } = renderRow({ invokeFn });
    const slider = getByTestId("ingest-gain-slider") as HTMLInputElement;

    const setNativeValue = Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      "value",
    )?.set;
    await act(async () => {
      setNativeValue?.call(slider, "2.5");
      fireEvent.input(slider, { target: { value: "2.5" } });
    });

    await waitFor(() => {
      expect(calls.some((c) => c.command === "set_capture_audio_controls")).toBe(true);
    });
    expect(calls).toContainEqual({
      command: "set_capture_audio_controls",
      args: { manual_ingest_gain: 2.5 },
    });
  });

  test("connected mode disables controls when capturePhase is not capturing", async () => {
    const { listenFn } = createMockListen();
    const { invokeFn } = createConnectedInvoke(
      { controls: defaultControls, ingest_level: null },
      "idle",
    );

    const { getByTestId } = render(
      <CaptureAudioControlsRow invokeFn={invokeFn} listenFn={listenFn} capturePhase="idle" />,
    );

    await waitFor(() => {
      const switchEl = getByTestId("mic-ingest-switch");
      expect(switchEl.hasAttribute("disabled")).toBe(true);
    });
    expect(getByTestId("ingest-level-meter").textContent).toBe("—");
  });

  test("connected mode shows dBFS when capturing with level from hook", async () => {
    const { listenFn } = createMockListen();
    const { invokeFn } = createConnectedInvoke({
      controls: defaultControls,
      ingest_level: activeLevel,
    });

    const { getByTestId } = render(
      <CaptureAudioControlsRow invokeFn={invokeFn} listenFn={listenFn} capturePhase="capturing" />,
    );

    await waitFor(() => {
      expect(getByTestId("ingest-level-meter").textContent).toBe("−18.2 dBFS");
    });
    expect((getByTestId("ingest-gain-slider") as HTMLInputElement).disabled).toBe(false);
  });
});
