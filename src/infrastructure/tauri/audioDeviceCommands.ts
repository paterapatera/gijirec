import { invoke } from "@tauri-apps/api/core";
import type { AudioDeviceList, DeviceSelection } from "../../presentation/hooks/audio-device-types";

export interface AudioDeviceCommandsOptions {
  invokeFn?: typeof invoke;
}

export async function listAudioDevices(
  options: AudioDeviceCommandsOptions = {},
): Promise<AudioDeviceList> {
  const { invokeFn = invoke } = options;
  return invokeFn<AudioDeviceList>("list_audio_devices");
}

export async function getDeviceSelection(
  options: AudioDeviceCommandsOptions = {},
): Promise<DeviceSelection> {
  const { invokeFn = invoke } = options;
  return invokeFn<DeviceSelection>("get_device_selection");
}

export async function setDeviceSelection(
  selection: DeviceSelection,
  options: AudioDeviceCommandsOptions = {},
): Promise<DeviceSelection> {
  const { invokeFn = invoke } = options;
  // Keys stay snake_case; host commands use `rename_all = "snake_case"`.
  return invokeFn<DeviceSelection>("set_device_selection", { ...selection });
}

export async function setAudioDeviceUiVisible(
  visible: boolean,
  options: AudioDeviceCommandsOptions = {},
): Promise<void> {
  const { invokeFn = invoke } = options;
  await invokeFn("set_audio_device_ui_visible", { visible });
}
