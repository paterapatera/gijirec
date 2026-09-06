/** Contract types per `docs/contracts/audio-device-selection.md`. */

export const DEVICES_CHANGED_EVENT = "audio-device-selection://devices-changed" as const;
export const SELECTION_CHANGED_EVENT = "audio-device-selection://selection-changed" as const;

/** cpal Device::name() — stable within a session; not restored across restarts. */
export type AudioDeviceId = string & { readonly __audioDeviceId?: never };

export type AudioDeviceKind = "input" | "output";

export interface AudioDeviceInfo {
  id: AudioDeviceId;
  /** OS-provided display name (req 1.3). */
  name: string;
  kind: AudioDeviceKind;
  /** OS default device for this kind. */
  is_default: boolean;
}

export interface DeviceSelection {
  /** null = OS default microphone (req 2.5–2.6). */
  microphone_id: AudioDeviceId | null;
  /** null = OS default output / loopback target (req 2.5–2.6). */
  speaker_id: AudioDeviceId | null;
}

export interface AudioDeviceList {
  inputs: AudioDeviceInfo[];
  outputs: AudioDeviceInfo[];
}

export interface AudioDevicesChanged {
  devices: AudioDeviceList;
  timestamp_ms: number;
}

export interface DeviceSelectionChanged {
  selection: DeviceSelection;
  timestamp_ms: number;
}

export type AudioDeviceUserErrorCode = "INVALID_DEVICE" | "MACOS_OUTPUT_NOT_DEFAULT" | "INTERNAL";

export interface AudioDeviceUserError {
  code: AudioDeviceUserErrorCode;
  message_ja: string;
  action_ja: string;
}

export interface AudioDevicesState {
  devices: AudioDeviceList;
  selection: DeviceSelection;
}

export const INITIAL_AUDIO_DEVICES_STATE: AudioDevicesState = {
  devices: { inputs: [], outputs: [] },
  selection: { microphone_id: null, speaker_id: null },
};

type AudioDeviceEventHandler = (event: {
  payload: AudioDevicesChanged | DeviceSelectionChanged;
}) => void;

export type AudioDeviceEventListenFn = (
  event: string,
  handler: AudioDeviceEventHandler,
) => Promise<() => void>;
