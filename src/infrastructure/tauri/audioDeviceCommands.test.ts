import { describe, expect, test } from "bun:test";
import type {
  AudioDeviceId,
  AudioDeviceInfo,
  AudioDeviceKind,
  AudioDeviceList,
  AudioDevicesChanged,
  AudioDeviceUserError,
  AudioDeviceUserErrorCode,
  DeviceSelection,
  DeviceSelectionChanged,
} from "../../presentation/hooks/audio-device-types";
import {
  DEVICES_CHANGED_EVENT,
  SELECTION_CHANGED_EVENT,
} from "../../presentation/hooks/audio-device-types";
import {
  getDeviceSelection,
  listAudioDevices,
  setAudioDeviceUiVisible,
  setDeviceSelection,
} from "./audioDeviceCommands";

type InvokeCall = {
  command: string;
  args?: unknown;
};

function createMockInvoke<T>(response: T) {
  const calls: InvokeCall[] = [];
  const invokeFn = async (command: string, args?: unknown) => {
    calls.push({ command, args });
    return response;
  };
  return { invokeFn, calls };
}

const inputKind: AudioDeviceKind = "input";
const outputKind: AudioDeviceKind = "output";

const builtInMicId: AudioDeviceId = "Built-in Microphone";
const usbMicId: AudioDeviceId = "USB Mic";
const builtInOutputId: AudioDeviceId = "Built-in Output";

const builtInMic: AudioDeviceInfo = {
  id: builtInMicId,
  name: builtInMicId,
  kind: inputKind,
  is_default: true,
};

const usbMic: AudioDeviceInfo = {
  id: usbMicId,
  name: usbMicId,
  kind: inputKind,
  is_default: false,
};

const builtInOutput: AudioDeviceInfo = {
  id: builtInOutputId,
  name: builtInOutputId,
  kind: outputKind,
  is_default: true,
};

const sampleDeviceList: AudioDeviceList = {
  inputs: [builtInMic, usbMic],
  outputs: [builtInOutput],
};

const sampleDevicesChanged: AudioDevicesChanged = {
  devices: sampleDeviceList,
  timestamp_ms: 1_700_000_000_000,
};

const sampleSelectionChanged: DeviceSelectionChanged = {
  selection: { microphone_id: usbMicId, speaker_id: null },
  timestamp_ms: 1_700_000_000_001,
};

const invalidDeviceCode: AudioDeviceUserErrorCode = "INVALID_DEVICE";

const sampleUserError: AudioDeviceUserError = {
  code: invalidDeviceCode,
  message_ja: "選択したデバイスが見つかりません",
  action_ja: "一覧を更新して別のデバイスを選んでください",
};

describe("audioDeviceCommands", () => {
  test("contract event names match audio-device-selection.md", () => {
    expect(DEVICES_CHANGED_EVENT).toBe("audio-device-selection://devices-changed");
    expect(SELECTION_CHANGED_EVENT).toBe("audio-device-selection://selection-changed");
    expect(sampleDevicesChanged.devices.inputs[0]?.kind).toBe(inputKind);
    expect(sampleSelectionChanged.selection.microphone_id).toBe(usbMicId);
    expect(sampleUserError.code).toBe(invalidDeviceCode);
  });

  test("listAudioDevices invokes list_audio_devices without args", async () => {
    const { invokeFn, calls } = createMockInvoke(sampleDeviceList);

    const actual = await listAudioDevices({ invokeFn });

    expect(calls).toEqual([{ command: "list_audio_devices" }]);
    expect(actual.inputs[0]?.kind).toBe("input");
    expect(actual.inputs[0]?.is_default).toBe(true);
    expect(actual.outputs[0]?.kind).toBe("output");
    expect(actual).toEqual(sampleDeviceList);
  });

  test("getDeviceSelection invokes get_device_selection without args", async () => {
    const selection: DeviceSelection = {
      microphone_id: "USB Mic",
      speaker_id: null,
    };
    const { invokeFn, calls } = createMockInvoke(selection);

    const actual = await getDeviceSelection({ invokeFn });

    expect(calls).toEqual([{ command: "get_device_selection" }]);
    expect(actual.microphone_id).toBe("USB Mic");
    expect(actual.speaker_id).toBeNull();
  });

  test("setDeviceSelection invokes set_device_selection with snake_case payload", async () => {
    const request: DeviceSelection = {
      microphone_id: null,
      speaker_id: "Built-in Output",
    };
    const response: DeviceSelection = {
      microphone_id: null,
      speaker_id: "Built-in Output",
    };
    const { invokeFn, calls } = createMockInvoke(response);

    const actual = await setDeviceSelection(request, { invokeFn });

    expect(calls).toEqual([
      {
        command: "set_device_selection",
        args: {
          microphone_id: null,
          speaker_id: "Built-in Output",
        },
      },
    ]);
    expect(actual).toEqual(response);
  });

  test("setAudioDeviceUiVisible invokes set_audio_device_ui_visible with visible flag", async () => {
    const { invokeFn, calls } = createMockInvoke(undefined);

    await setAudioDeviceUiVisible(true, { invokeFn });

    expect(calls).toEqual([
      {
        command: "set_audio_device_ui_visible",
        args: { visible: true },
      },
    ]);
  });

  test("setAudioDeviceUiVisible passes visible false when UI unmounts", async () => {
    const { invokeFn, calls } = createMockInvoke(undefined);

    await setAudioDeviceUiVisible(false, { invokeFn });

    expect(calls).toEqual([
      {
        command: "set_audio_device_ui_visible",
        args: { visible: false },
      },
    ]);
  });
});
