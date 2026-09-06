import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { setupTestDom } from "../../test-setup";
import type {
  AudioDeviceEventListenFn,
  AudioDeviceInfo,
  AudioDeviceKind,
  AudioDeviceList,
  AudioDevicesChanged,
  DeviceSelection,
  DeviceSelectionChanged,
} from "./audio-device-types";
import {
  DEVICES_CHANGED_EVENT,
  INITIAL_AUDIO_DEVICES_STATE,
  SELECTION_CHANGED_EVENT,
} from "./audio-device-types";
import { useAudioDevices } from "./useAudioDevices";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
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

  const listenFn: AudioDeviceEventListenFn = async (event, handler) => {
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
  return { invokeFn, calls };
}

const inputKind: AudioDeviceKind = "input";
const outputKind: AudioDeviceKind = "output";

const builtInMic: AudioDeviceInfo = {
  id: "Built-in Microphone",
  name: "Built-in Microphone",
  kind: inputKind,
  is_default: true,
};

const usbMic: AudioDeviceInfo = {
  id: "USB Mic",
  name: "USB Mic",
  kind: inputKind,
  is_default: false,
};

const builtInOutput: AudioDeviceInfo = {
  id: "Built-in Output",
  name: "Built-in Output",
  kind: outputKind,
  is_default: true,
};

const sampleDeviceList: AudioDeviceList = {
  inputs: [builtInMic, usbMic],
  outputs: [builtInOutput],
};

const sampleSelection: DeviceSelection = {
  microphone_id: usbMic.id,
  speaker_id: null,
};

describe("useAudioDevices", () => {
  test("starts with empty devices and null selection", () => {
    const { listenFn } = createMockListen();
    const { invokeFn } = createMockInvoke({
      set_audio_device_ui_visible: () => undefined,
    });
    const { result } = renderHook(() => useAudioDevices({ listenFn, invokeFn }));

    expect(result.current).toEqual(INITIAL_AUDIO_DEVICES_STATE);
  });

  test("syncs device list and selection from invoke on mount", async () => {
    const { listenFn } = createMockListen();
    const { invokeFn } = createMockInvoke({
      list_audio_devices: () => sampleDeviceList,
      get_device_selection: () => sampleSelection,
      set_audio_device_ui_visible: () => undefined,
    });

    const { result } = renderHook(() => useAudioDevices({ listenFn, invokeFn }));

    await waitFor(() => {
      expect(result.current.devices.inputs).toHaveLength(2);
    });
    expect(result.current.devices).toEqual(sampleDeviceList);
    expect(result.current.selection).toEqual(sampleSelection);
  });

  test("sets audio device UI visible on mount and false on unmount", async () => {
    const { listenFn } = createMockListen();
    const { invokeFn, calls } = createMockInvoke({
      list_audio_devices: () => sampleDeviceList,
      get_device_selection: () => sampleSelection,
      set_audio_device_ui_visible: () => undefined,
    });

    const { unmount } = renderHook(() => useAudioDevices({ listenFn, invokeFn }));

    await waitFor(() => {
      expect(calls.some((c) => c.command === "list_audio_devices")).toBe(true);
    });

    expect(calls).toContainEqual({
      command: "set_audio_device_ui_visible",
      args: { visible: true },
    });

    unmount();

    await waitFor(() => {
      expect(calls).toContainEqual({
        command: "set_audio_device_ui_visible",
        args: { visible: false },
      });
    });
  });

  test("updates devices when devices-changed is emitted", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { invokeFn } = createMockInvoke({
      list_audio_devices: () => ({ inputs: [], outputs: [] }),
      get_device_selection: () => ({ microphone_id: null, speaker_id: null }),
      set_audio_device_ui_visible: () => undefined,
    });

    const { result } = renderHook(() => useAudioDevices({ listenFn, invokeFn }));

    await waitFor(() => {
      expect(listeners.has(DEVICES_CHANGED_EVENT)).toBe(true);
    });

    const payload: AudioDevicesChanged = {
      devices: sampleDeviceList,
      timestamp_ms: 1_700_000_000_000,
    };
    act(() => {
      emit(DEVICES_CHANGED_EVENT, payload);
    });

    await waitFor(() => {
      expect(result.current.devices).toEqual(sampleDeviceList);
    });
    expect(result.current.selection).toEqual({ microphone_id: null, speaker_id: null });
  });

  test("updates selection when selection-changed is emitted", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { invokeFn } = createMockInvoke({
      list_audio_devices: () => sampleDeviceList,
      get_device_selection: () => ({ microphone_id: null, speaker_id: null }),
      set_audio_device_ui_visible: () => undefined,
    });

    const { result } = renderHook(() => useAudioDevices({ listenFn, invokeFn }));

    await waitFor(() => {
      expect(listeners.has(SELECTION_CHANGED_EVENT)).toBe(true);
    });

    const payload: DeviceSelectionChanged = {
      selection: sampleSelection,
      timestamp_ms: 1_700_000_000_001,
    };
    act(() => {
      emit(SELECTION_CHANGED_EVENT, payload);
    });

    await waitFor(() => {
      expect(result.current.selection).toEqual(sampleSelection);
    });
    expect(result.current.devices).toEqual(sampleDeviceList);
  });

  test("unmount unlistens from both events", async () => {
    const { listenFn, unlistenEvents, listeners } = createMockListen();
    const { invokeFn } = createMockInvoke({
      list_audio_devices: () => sampleDeviceList,
      get_device_selection: () => sampleSelection,
      set_audio_device_ui_visible: () => undefined,
    });

    const { unmount } = renderHook(() => useAudioDevices({ listenFn, invokeFn }));

    await waitFor(() => {
      expect(listeners.has(DEVICES_CHANGED_EVENT)).toBe(true);
      expect(listeners.has(SELECTION_CHANGED_EVENT)).toBe(true);
    });

    unmount();

    await waitFor(() => {
      expect(unlistenEvents).toContain(DEVICES_CHANGED_EVENT);
      expect(unlistenEvents).toContain(SELECTION_CHANGED_EVENT);
    });
  });
});
