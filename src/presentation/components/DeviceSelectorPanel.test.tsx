import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { setupTestDom } from "../../test-setup";
import type {
  AudioDeviceInfo,
  AudioDeviceList,
  DeviceSelection,
} from "../hooks/audio-device-types";
import type { CaptureUserError } from "../hooks/capture-status";
import { DeviceSelectorPanel } from "./DeviceSelectorPanel";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

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

const nullSelection: DeviceSelection = {
  microphone_id: null,
  speaker_id: null,
};

const sampleError: CaptureUserError = {
  code: "DEVICE_DISCONNECTED",
  message_ja: "選択したデバイスが切断されました",
  action_ja: "別のデバイスを選択してください",
  recoverable: true,
};

function renderPanel(
  overrides: {
    devices?: AudioDeviceList;
    selection?: DeviceSelection;
    captureError?: CaptureUserError | null;
    onSelectionChange?: (selection: DeviceSelection) => void;
    isMacos?: boolean;
    detectMacos?: () => boolean;
    invokeFn?: (command: string, args?: unknown) => Promise<unknown>;
  } = {},
) {
  const props: Record<string, unknown> = {
    devices: overrides.devices ?? sampleDevices,
    selection: overrides.selection ?? nullSelection,
    captureError: overrides.captureError ?? null,
  };
  if (overrides.onSelectionChange !== undefined) {
    props.onSelectionChange = overrides.onSelectionChange;
  }
  if (overrides.isMacos !== undefined) {
    props.isMacos = overrides.isMacos;
  }
  if (overrides.detectMacos !== undefined) {
    props.detectMacos = overrides.detectMacos;
  }
  if (overrides.invokeFn !== undefined) {
    props.invokeFn = overrides.invokeFn;
  }
  return render(<DeviceSelectorPanel {...props} />);
}

describe("DeviceSelectorPanel", () => {
  // E2E/UI 1 (req 2.5): OS 既定表示
  test("shows OS 既定 as selected when selection ids are null", () => {
    const { getByTestId } = renderPanel({ detectMacos: () => false });

    const micSelect = getByTestId("microphone-select") as HTMLSelectElement;
    const speakerSelect = getByTestId("speaker-select") as HTMLSelectElement;

    expect(micSelect.value).toBe("");
    expect(speakerSelect.value).toBe("");
    expect(micSelect.options[0]?.text).toBe("OS 既定");
    expect(micSelect.options[0]?.selected).toBe(true);
  });

  test("shows current device ids when selection is set", () => {
    const { getByTestId } = renderPanel({
      selection: { microphone_id: usbMic.id, speaker_id: hdmiSpeaker.id },
      detectMacos: () => false,
    });

    const micSelect = getByTestId("microphone-select") as HTMLSelectElement;
    const speakerSelect = getByTestId("speaker-select") as HTMLSelectElement;

    expect(micSelect.value).toBe(usbMic.id);
    expect(speakerSelect.value).toBe(hdmiSpeaker.id);
  });

  // E2E/UI 3 (req 1.5): 候補ゼロ empty state
  test("shows empty state when microphone candidates are zero", () => {
    const { getByTestId, queryByTestId } = renderPanel({
      devices: { inputs: [], outputs: sampleDevices.outputs },
      detectMacos: () => false,
    });

    expect(getByTestId("microphone-empty").textContent).toBe("利用可能なマイクがありません");
    expect(queryByTestId("microphone-select")).toBeNull();
  });

  test("shows empty state when speaker candidates are zero", () => {
    const { getByTestId, queryByTestId } = renderPanel({
      devices: { inputs: sampleDevices.inputs, outputs: [] },
      detectMacos: () => false,
    });

    expect(getByTestId("speaker-empty").textContent).toBe("利用可能なスピーカーがありません");
    expect(queryByTestId("speaker-select")).toBeNull();
  });

  // E2E/UI 5 (req 6.1, ADR-0009): macOS スピーカー案内
  test("shows macOS speaker help text when isMacos is true", () => {
    const { getByTestId } = renderPanel({ isMacos: true });

    expect(getByTestId("speaker-macos-help").textContent).toContain(
      "macOSでは、システム音声の取得には選択したスピーカーがOSの既定出力と一致している必要があります",
    );
  });

  test("hides macOS speaker help text when isMacos is false", () => {
    const { queryByTestId } = renderPanel({ isMacos: false });

    expect(queryByTestId("speaker-macos-help")).toBeNull();
  });

  test("shows macOS speaker help when isMacos omitted and detectMacos returns true", () => {
    const { getByTestId } = renderPanel({ detectMacos: () => true });

    expect(getByTestId("speaker-macos-help").textContent).toContain(
      "macOSでは、システム音声の取得には選択したスピーカーがOSの既定出力と一致している必要があります",
    );
  });

  test("hides macOS speaker help when isMacos omitted and detectMacos returns false", () => {
    const { queryByTestId } = renderPanel({ detectMacos: () => false });

    expect(queryByTestId("speaker-macos-help")).toBeNull();
  });

  test("detects macOS from navigator.userAgent when isMacos omitted", () => {
    const original = navigator.userAgent;
    Object.defineProperty(navigator, "userAgent", {
      value: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
      configurable: true,
    });
    try {
      const { getByTestId } = renderPanel();
      expect(getByTestId("speaker-macos-help").textContent).toContain(
        "macOSでは、システム音声の取得には選択したスピーカーがOSの既定出力と一致している必要があります",
      );
    } finally {
      Object.defineProperty(navigator, "userAgent", {
        value: original,
        configurable: true,
      });
    }
  });

  // E2E/UI 4 (req 4.4): action_ja 含有
  test("shows capture error message and action_ja", () => {
    const { getByTestId } = renderPanel({ captureError: sampleError, detectMacos: () => false });

    expect(getByTestId("device-error-message").textContent).toBe(sampleError.message_ja);
    expect(getByTestId("device-error-action").textContent).toBe(sampleError.action_ja);
  });

  test("keeps selects operable when capture error is shown", () => {
    const changes: DeviceSelection[] = [];
    const { getByTestId } = renderPanel({
      captureError: sampleError,
      detectMacos: () => false,
      onSelectionChange: (selection) => {
        changes.push(selection);
      },
    });

    fireEvent.change(getByTestId("microphone-select"), {
      target: { value: usbMic.id },
    });

    expect(changes).toEqual([{ microphone_id: usbMic.id, speaker_id: null }]);
    expect(getByTestId("microphone-select").hasAttribute("disabled")).toBe(false);
    expect(getByTestId("speaker-select").hasAttribute("disabled")).toBe(false);
  });

  test("calls set_device_selection via invokeFn when onSelectionChange is omitted", async () => {
    const calls: { command: string; args?: unknown }[] = [];
    const invokeFn = async (command: string, args?: unknown) => {
      calls.push({ command, args });
      return args;
    };

    const { getByTestId } = renderPanel({
      detectMacos: () => false,
      invokeFn,
    });

    fireEvent.change(getByTestId("microphone-select"), {
      target: { value: usbMic.id },
    });

    await waitFor(() => {
      expect(calls.some((call) => call.command === "set_device_selection")).toBe(true);
    });
    expect(calls).toContainEqual({
      command: "set_device_selection",
      args: { microphone_id: usbMic.id, speaker_id: null },
    });
  });

  test("calls onSelectionChange when microphone selection changes", () => {
    const changes: DeviceSelection[] = [];
    const { getByTestId } = renderPanel({
      detectMacos: () => false,
      onSelectionChange: (selection) => {
        changes.push(selection);
      },
    });

    fireEvent.change(getByTestId("microphone-select"), {
      target: { value: usbMic.id },
    });

    expect(changes).toEqual([{ microphone_id: usbMic.id, speaker_id: null }]);
  });

  test("calls onSelectionChange with null when OS 既定 is chosen", () => {
    const changes: DeviceSelection[] = [];
    const { getByTestId } = renderPanel({
      selection: { microphone_id: usbMic.id, speaker_id: hdmiSpeaker.id },
      detectMacos: () => false,
      onSelectionChange: (selection) => {
        changes.push(selection);
      },
    });

    fireEvent.change(getByTestId("microphone-select"), {
      target: { value: "" },
    });

    expect(changes).toEqual([{ microphone_id: null, speaker_id: hdmiSpeaker.id }]);
  });

  test("calls onSelectionChange when speaker selection changes", () => {
    const changes: DeviceSelection[] = [];
    const { getByTestId } = renderPanel({
      detectMacos: () => false,
      onSelectionChange: (selection) => {
        changes.push(selection);
      },
    });

    fireEvent.change(getByTestId("speaker-select"), {
      target: { value: hdmiSpeaker.id },
    });

    expect(changes).toEqual([{ microphone_id: null, speaker_id: hdmiSpeaker.id }]);
  });
});
