import type { AudioDeviceList, DeviceSelection } from "../../presentation/hooks/audio-device-types";
import { defaultInvoke, type InjectableInvokeFn } from "./injectableInvoke";

export interface AudioDeviceCommandsOptions {
  invokeFn?: InjectableInvokeFn;
}

export async function listAudioDevices(
  options: AudioDeviceCommandsOptions = {},
): Promise<AudioDeviceList> {
  const { invokeFn = defaultInvoke } = options;
  return invokeFn<AudioDeviceList>("list_audio_devices");
}

export async function getDeviceSelection(
  options: AudioDeviceCommandsOptions = {},
): Promise<DeviceSelection> {
  const { invokeFn = defaultInvoke } = options;
  return invokeFn<DeviceSelection>("get_device_selection");
}

export async function setDeviceSelection(
  selection: DeviceSelection,
  options: AudioDeviceCommandsOptions = {},
): Promise<DeviceSelection> {
  const { invokeFn = defaultInvoke } = options;
  // Keys stay snake_case; host commands use `rename_all = "snake_case"`.
  return invokeFn<DeviceSelection>("set_device_selection", { ...selection });
}

export async function setAudioDeviceUiVisible(
  visible: boolean,
  options: AudioDeviceCommandsOptions = {},
): Promise<void> {
  const { invokeFn = defaultInvoke } = options;
  await invokeFn("set_audio_device_ui_visible", { visible });
}
