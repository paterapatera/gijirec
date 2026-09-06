import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Dispatch, SetStateAction } from "react";
import { useEffect, useState } from "react";
import {
  getDeviceSelection,
  listAudioDevices,
  setAudioDeviceUiVisible,
} from "../../infrastructure/tauri/audioDeviceCommands";
import type {
  AudioDeviceEventListenFn,
  AudioDeviceList,
  AudioDevicesChanged,
  AudioDevicesState,
  DeviceSelection,
  DeviceSelectionChanged,
} from "./audio-device-types";
import {
  DEVICES_CHANGED_EVENT,
  INITIAL_AUDIO_DEVICES_STATE,
  SELECTION_CHANGED_EVENT,
} from "./audio-device-types";

export interface UseAudioDevicesOptions {
  listenFn?: AudioDeviceEventListenFn;
  invokeFn?: typeof invoke;
}

function isAudioDeviceList(value: unknown): value is AudioDeviceList {
  if (value === null || typeof value !== "object") {
    return false;
  }
  const record = value as { inputs?: unknown; outputs?: unknown };
  return Array.isArray(record.inputs) && Array.isArray(record.outputs);
}

function isDeviceSelection(value: unknown): value is DeviceSelection {
  if (value === null || typeof value !== "object") {
    return false;
  }
  return "microphone_id" in value && "speaker_id" in value;
}

function applyDevicesChanged(
  payload: AudioDevicesChanged,
  setState: Dispatch<SetStateAction<AudioDevicesState>>,
): void {
  setState((prev) => ({
    ...prev,
    devices: payload.devices,
  }));
}

function applySelectionChanged(
  payload: DeviceSelectionChanged,
  setState: Dispatch<SetStateAction<AudioDevicesState>>,
): void {
  setState((prev) => ({
    ...prev,
    selection: payload.selection,
  }));
}

async function syncInitialData(
  invokeFn: typeof invoke,
  setState: Dispatch<SetStateAction<AudioDevicesState>>,
): Promise<void> {
  try {
    const [devicesRaw, selectionRaw] = await Promise.all([
      listAudioDevices({ invokeFn }),
      getDeviceSelection({ invokeFn }),
    ]);
    const devicesUnknown: unknown = devicesRaw;
    const selectionUnknown: unknown = selectionRaw;
    const devices = isAudioDeviceList(devicesUnknown)
      ? devicesUnknown
      : INITIAL_AUDIO_DEVICES_STATE.devices;
    const selection = isDeviceSelection(selectionUnknown)
      ? selectionUnknown
      : INITIAL_AUDIO_DEVICES_STATE.selection;
    setState({ devices, selection });
  } catch (error) {
    // Browser-only Vite has no Tauri IPC. ACL denials also land here — do not hide them in the app shell.
    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      console.error("list_audio_devices / get_device_selection failed", error);
    }
  }
}

async function setUiVisible(visible: boolean, invokeFn: typeof invoke): Promise<void> {
  try {
    await setAudioDeviceUiVisible(visible, { invokeFn });
  } catch {
    // Browser-only dev (no Tauri shell).
  }
}

async function subscribeAudioDeviceEvents(
  listenFn: AudioDeviceEventListenFn,
  setState: Dispatch<SetStateAction<AudioDevicesState>>,
  isCancelled: () => boolean,
): Promise<{ unlistenDevices: () => void; unlistenSelection: () => void } | undefined> {
  const unlistenDevices = await listenFn(DEVICES_CHANGED_EVENT, (event) => {
    applyDevicesChanged(event.payload as AudioDevicesChanged, setState);
  });
  if (isCancelled()) {
    unlistenDevices();
    return undefined;
  }

  const unlistenSelection = await listenFn(SELECTION_CHANGED_EVENT, (event) => {
    applySelectionChanged(event.payload as DeviceSelectionChanged, setState);
  });
  if (isCancelled()) {
    unlistenDevices();
    unlistenSelection();
    return undefined;
  }

  return { unlistenDevices, unlistenSelection };
}

/**
 * Mirrors audio device list/selection from Tauri commands and selection events.
 * Marks UI visible while mounted so the backend can emit hot-plug updates (req 5.1).
 */
export function useAudioDevices(options: UseAudioDevicesOptions = {}): AudioDevicesState {
  const { listenFn = listen, invokeFn = invoke } = options;
  const [state, setState] = useState<AudioDevicesState>(INITIAL_AUDIO_DEVICES_STATE);

  useEffect(() => {
    let cancelled = false;
    let cleanupListeners: (() => void) | undefined;

    void setUiVisible(true, invokeFn);
    void syncInitialData(invokeFn, setState);
    void subscribeAudioDeviceEvents(listenFn, setState, () => cancelled).then((handles) => {
      if (handles === undefined) {
        return;
      }
      cleanupListeners = () => {
        handles.unlistenDevices();
        handles.unlistenSelection();
      };
      if (cancelled) {
        cleanupListeners();
      }
    });

    return () => {
      cancelled = true;
      cleanupListeners?.();
      void setUiVisible(false, invokeFn);
    };
  }, [invokeFn, listenFn]);

  return state;
}
