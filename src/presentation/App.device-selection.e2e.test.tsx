/**
 * audio-device-selection E2E/UI tests (design Testing Strategy E2E/UI 1–5, Wave 25 / Task 9.4).
 * Panel-level cases 1, 3, 4, 5 are also covered in DeviceSelectorPanel.test.tsx.
 */
import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { act, cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { setupTestDom } from "../test-setup";
import { App } from "./App";
import type { AudioDeviceInfo, AudioDeviceList, DeviceSelection } from "./hooks/audio-device-types";
import { SELECTION_CHANGED_EVENT } from "./hooks/audio-device-types";
import type { CaptureEventListenFn, CaptureUserError } from "./hooks/capture-status";
import { ERROR_EVENT, PHASE_CHANGED_EVENT } from "./hooks/capture-status";
import type { TranscribeEventListenFn } from "./hooks/transcribe-status";
import { handleCommonTranscribeInvokeCommands } from "./testInvokeHelpers";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

type EventHandler = (event: { payload: unknown }) => void;
type InvokeCall = { cmd: string; args?: Record<string, unknown> };

const builtInMic: AudioDeviceInfo = {
  id: "Built-in Microphone",
  name: "Built-in Microphone",
  kind: "input",
  is_default: true,
};

const usbMic: AudioDeviceInfo = {
  id: "USB Microphone",
  name: "USB Microphone",
  kind: "input",
  is_default: false,
};

const builtInSpeaker: AudioDeviceInfo = {
  id: "Built-in Output",
  name: "Built-in Output",
  kind: "output",
  is_default: true,
};

const hdmiSpeaker: AudioDeviceInfo = {
  id: "HDMI Output",
  name: "HDMI Output",
  kind: "output",
  is_default: false,
};

const sampleDevices: AudioDeviceList = {
  inputs: [builtInMic, usbMic],
  outputs: [builtInSpeaker, hdmiSpeaker],
};

function createMockListen() {
  const listeners = new Map<string, EventHandler[]>();

  const listenFn: CaptureEventListenFn & TranscribeEventListenFn = async (event, handler) => {
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

function createDeviceSelectionInvoke(
  options: { devices?: AudioDeviceList; initialSelection?: DeviceSelection } = {},
) {
  const devices = options.devices ?? sampleDevices;
  let selection: DeviceSelection = options.initialSelection ?? {
    microphone_id: null,
    speaker_id: null,
  };
  const calls: InvokeCall[] = [];

  const invokeFn = async (cmd: string, args?: Record<string, unknown>) => {
    calls.push({ cmd, args });
    const transcribe = handleCommonTranscribeInvokeCommands(cmd);
    if (transcribe !== undefined) {
      return transcribe;
    }
    switch (cmd) {
      case "list_audio_devices":
        return devices;
      case "get_device_selection":
        return selection;
      case "set_device_selection":
        selection = {
          microphone_id: (args?.microphone_id as string | null | undefined) ?? null,
          speaker_id: (args?.speaker_id as string | null | undefined) ?? null,
        };
        return selection;
      case "set_audio_device_ui_visible":
        return;
      case "get_editor_settings":
        return { save_directory: null, export_jsonl_enabled: false };
      default:
        return {};
    }
  };

  return { invokeFn, calls, getSelection: () => ({ ...selection }) };
}

describe("audio-device-selection E2E/UI 1: OS default on startup (req 2.5)", () => {
  test("App loads device list and shows OS 既定 for null selection", async () => {
    const { listenFn } = createMockListen();
    const mock = createDeviceSelectionInvoke();
    const { getByTestId } = render(<App listenFn={listenFn} invokeFn={mock.invokeFn} />);

    await waitFor(() => {
      expect(mock.calls.some((c) => c.cmd === "list_audio_devices")).toBe(true);
      expect(mock.calls.some((c) => c.cmd === "get_device_selection")).toBe(true);
      expect(
        mock.calls.some((c) => c.cmd === "set_audio_device_ui_visible" && c.args?.visible === true),
      ).toBe(true);
      const micSelect = getByTestId("microphone-select") as HTMLSelectElement;
      expect(micSelect.options.length).toBeGreaterThan(1);
    });

    const micSelect = getByTestId("microphone-select") as HTMLSelectElement;
    const speakerSelect = getByTestId("speaker-select") as HTMLSelectElement;

    expect(micSelect.value).toBe("");
    expect(speakerSelect.value).toBe("");
    expect(micSelect.options[0]?.text).toBe("OS 既定");
    expect(speakerSelect.options[0]?.text).toBe("OS 既定");
    expect(micSelect.options[0]?.selected).toBe(true);
    expect(speakerSelect.options[0]?.selected).toBe(true);
    expect(
      Array.from(micSelect.options).some(
        (option) => option.value === usbMic.id && option.text === usbMic.name,
      ),
    ).toBe(true);
    expect(
      Array.from(speakerSelect.options).some(
        (option) => option.value === hdmiSpeaker.id && option.text === hdmiSpeaker.name,
      ),
    ).toBe(true);
  });
});

describe("audio-device-selection E2E/UI 2: mic change while capturing (req 3.1, 3.3)", () => {
  test("changing microphone invokes set_device_selection and keeps capturing phase", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const mock = createDeviceSelectionInvoke();
    const { getByTestId } = render(<App listenFn={listenFn} invokeFn={mock.invokeFn} />);

    await waitFor(() => {
      expect(getByTestId("microphone-select")).toBeTruthy();
      expect(listeners.has(PHASE_CHANGED_EVENT)).toBe(true);
    });

    act(() => {
      emit(PHASE_CHANGED_EVENT, { phase: "capturing", timestamp_ms: 1_000 });
    });

    await waitFor(() => {
      expect(getByTestId("capture-phase").textContent).toBe("capturing");
    });

    await act(async () => {
      fireEvent.change(getByTestId("microphone-select"), {
        target: { value: usbMic.id },
      });
    });

    await waitFor(() => {
      expect(mock.calls.some((c) => c.cmd === "set_device_selection")).toBe(true);
    });

    expect(mock.getSelection()).toEqual({
      microphone_id: usbMic.id,
      speaker_id: null,
    });
    expect(getByTestId("capture-phase").textContent).toBe("capturing");

    act(() => {
      emit(PHASE_CHANGED_EVENT, { phase: "capturing", timestamp_ms: 2_000 });
      emit(SELECTION_CHANGED_EVENT, {
        selection: { microphone_id: usbMic.id, speaker_id: null },
        timestamp_ms: 2_000,
      });
    });

    await waitFor(() => {
      const micSelect = getByTestId("microphone-select") as HTMLSelectElement;
      expect(micSelect.value).toBe(usbMic.id);
      expect(getByTestId("capture-phase").textContent).toBe("capturing");
    });
  });
});

describe("audio-device-selection E2E/UI 3: empty state (req 1.5)", () => {
  test("App shows empty state when device list has zero candidates", async () => {
    const { listenFn } = createMockListen();
    const mock = createDeviceSelectionInvoke({
      devices: { inputs: [], outputs: [] },
    });
    const { getByTestId, queryByTestId } = render(
      <App listenFn={listenFn} invokeFn={mock.invokeFn} />,
    );

    await waitFor(() => {
      expect(getByTestId("microphone-empty").textContent).toBe("利用可能なマイクがありません");
      expect(getByTestId("speaker-empty").textContent).toBe("利用可能なスピーカーがありません");
    });

    expect(queryByTestId("microphone-select")).toBeNull();
    expect(queryByTestId("speaker-select")).toBeNull();
  });
});

describe("audio-device-selection E2E/UI 4: action_ja in device error (req 4.4)", () => {
  test("App device panel shows message_ja and action_ja on capture error event", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const mock = createDeviceSelectionInvoke();
    const { getByTestId, container } = render(<App listenFn={listenFn} invokeFn={mock.invokeFn} />);

    await waitFor(() => {
      expect(listeners.has(ERROR_EVENT)).toBe(true);
      expect(getByTestId("microphone-select")).toBeTruthy();
    });

    const payload: CaptureUserError = {
      code: "DEVICE_DISCONNECTED",
      message_ja: "選択したデバイスが切断されました",
      action_ja: "別のデバイスを選択してください",
      recoverable: true,
    };

    act(() => {
      emit(ERROR_EVENT, payload);
    });

    await waitFor(() => {
      expect(getByTestId("device-error-message").textContent).toBe(payload.message_ja);
      expect(getByTestId("device-error-action").textContent).toBe(payload.action_ja);
    });

    expect(getByTestId("microphone-select").hasAttribute("disabled")).toBe(false);
    expect(container.textContent).not.toContain("DEVICE_DISCONNECTED");
  });
});

describe("audio-device-selection E2E/UI 5: macOS speaker guidance (req 6.1, ADR-0009)", () => {
  test("App shows macOS speaker help when platform is macOS", async () => {
    const original = navigator.userAgent;
    Object.defineProperty(navigator, "userAgent", {
      value: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
      configurable: true,
    });

    try {
      const { listenFn } = createMockListen();
      const mock = createDeviceSelectionInvoke();
      const { getByTestId } = render(<App listenFn={listenFn} invokeFn={mock.invokeFn} />);

      await waitFor(() => {
        expect(getByTestId("speaker-macos-help").textContent).toContain(
          "macOSでは、システム音声の取得には選択したスピーカーがOSの既定出力と一致している必要があります",
        );
      });

      await act(async () => {
        fireEvent.change(getByTestId("speaker-select"), {
          target: { value: hdmiSpeaker.id },
        });
      });

      await waitFor(() => {
        expect(getByTestId("speaker-macos-help")).toBeTruthy();
        expect(mock.getSelection().speaker_id).toBe(hdmiSpeaker.id);
      });
    } finally {
      Object.defineProperty(navigator, "userAgent", {
        value: original,
        configurable: true,
      });
    }
  });
});
