import type { invoke } from "@tauri-apps/api/core";
import type { ChangeEvent } from "react";
import { setDeviceSelection } from "../../infrastructure/tauri/audioDeviceCommands";
import type {
  AudioDeviceEventListenFn,
  AudioDeviceInfo,
  AudioDeviceList,
  DeviceSelection,
} from "../hooks/audio-device-types";
import type { CaptureAudioControlsEventListenFn } from "../hooks/capture-audio-controls-types";
import type {
  CaptureEventListenFn,
  CapturePhaseChanged,
  CaptureUserError,
} from "../hooks/capture-status";
import { useAudioDevices } from "../hooks/useAudioDevices";
import { useCaptureStatus } from "../hooks/useCaptureStatus";
import { CaptureAudioControlsRow } from "./CaptureAudioControlsRow";
import { detectMacos as defaultDetectMacos } from "./detectMacos";

const OS_DEFAULT_VALUE = "";

const MACOS_SPEAKER_HELP =
  "macOSでは、システム音声の取得には選択したスピーカーがOSの既定出力と一致している必要があります。一致しない場合は、システム設定で出力デバイスを変更するか、「OS 既定」を選択してください。";

const MICROPHONE_EMPTY_TEXT = "利用可能なマイクがありません";
const SPEAKER_EMPTY_TEXT = "利用可能なスピーカーがありません";

interface DeviceSelectorPanelInjectedProps {
  readonly devices: AudioDeviceList;
  readonly selection: DeviceSelection;
  readonly captureError: CaptureUserError | null;
  readonly onSelectionChange?: (selection: DeviceSelection) => void;
  readonly isMacos?: boolean;
}

interface DeviceSelectorPanelRuntimeProps {
  readonly detectMacos?: () => boolean;
  readonly invokeFn?: typeof invoke;
  readonly listenFn?: (
    event: string,
    handler: (event: { payload: unknown }) => void,
  ) => Promise<() => void>;
  readonly captureListenFn?: CaptureEventListenFn;
}

export type DeviceSelectorPanelProps = Partial<DeviceSelectorPanelInjectedProps> &
  DeviceSelectorPanelRuntimeProps;

interface DeviceSelectFieldProps {
  readonly id: string;
  readonly label: string;
  readonly testId: string;
  readonly emptyTestId: string;
  readonly emptyText: string;
  readonly devices: AudioDeviceInfo[];
  readonly selectedId: string | null;
  readonly onChange: (event: ChangeEvent<HTMLSelectElement>) => void;
  readonly helpText?: string;
  readonly helpTestId?: string;
}

function DeviceSelectField({
  id,
  label,
  testId,
  emptyTestId,
  emptyText,
  devices,
  selectedId,
  onChange,
  helpText,
  helpTestId,
}: DeviceSelectFieldProps) {
  return (
    <div className="device-field">
      <label className="status-label" htmlFor={id}>
        {label}
      </label>
      {helpText !== undefined ? (
        <p className="device-help-text" data-testid={helpTestId}>
          {helpText}
        </p>
      ) : null}
      {devices.length === 0 ? (
        <p className="device-empty-state" data-testid={emptyTestId}>
          {emptyText}
        </p>
      ) : (
        <select
          id={id}
          className="device-select"
          data-testid={testId}
          value={selectedId ?? OS_DEFAULT_VALUE}
          onChange={onChange}
        >
          <option value={OS_DEFAULT_VALUE}>OS 既定</option>
          {devices.map((device) => (
            <option key={device.id} value={device.id}>
              {device.name}
            </option>
          ))}
        </select>
      )}
    </div>
  );
}

interface CaptureErrorDisplayProps {
  readonly error: CaptureUserError;
}

function CaptureErrorDisplay({ error }: CaptureErrorDisplayProps) {
  return (
    <section className="error-panel" role="alert">
      <p className="error-message" data-testid="device-error-message">
        {error.message_ja}
      </p>
      <p className="error-action" data-testid="device-error-action">
        {error.action_ja}
      </p>
    </section>
  );
}

type CapturePhase = CapturePhaseChanged["phase"];

interface DeviceSelectorPanelViewProps extends DeviceSelectorPanelInjectedProps {
  readonly capturePhase?: CapturePhase;
  readonly invokeFn?: typeof invoke;
  readonly captureAudioControlsListenFn?: CaptureAudioControlsEventListenFn;
}

function DeviceSelectorPanelView({
  devices,
  selection,
  captureError,
  onSelectionChange,
  isMacos,
  capturePhase,
  invokeFn,
  captureAudioControlsListenFn,
}: DeviceSelectorPanelViewProps) {
  const applySelection = (next: DeviceSelection): void => {
    if (onSelectionChange !== undefined) {
      onSelectionChange(next);
      return;
    }
    void setDeviceSelection(next, invokeFn !== undefined ? { invokeFn } : {});
  };

  const handleMicrophoneChange = (event: ChangeEvent<HTMLSelectElement>): void => {
    const microphone_id = event.target.value === OS_DEFAULT_VALUE ? null : event.target.value;
    applySelection({ ...selection, microphone_id });
  };

  const handleSpeakerChange = (event: ChangeEvent<HTMLSelectElement>): void => {
    const speaker_id = event.target.value === OS_DEFAULT_VALUE ? null : event.target.value;
    applySelection({ ...selection, speaker_id });
  };

  return (
    <section className="status-panel device-selector-panel" aria-label="オーディオデバイス選択">
      <DeviceSelectField
        id="microphone-select"
        label="マイク"
        testId="microphone-select"
        emptyTestId="microphone-empty"
        emptyText={MICROPHONE_EMPTY_TEXT}
        devices={devices.inputs}
        selectedId={selection.microphone_id}
        onChange={handleMicrophoneChange}
      />
      <DeviceSelectField
        id="speaker-select"
        label="スピーカー"
        testId="speaker-select"
        emptyTestId="speaker-empty"
        emptyText={SPEAKER_EMPTY_TEXT}
        devices={devices.outputs}
        selectedId={selection.speaker_id}
        onChange={handleSpeakerChange}
        {...(isMacos ? { helpText: MACOS_SPEAKER_HELP, helpTestId: "speaker-macos-help" } : {})}
      />
      <CaptureAudioControlsRow
        {...(capturePhase !== undefined ? { capturePhase } : {})}
        {...(invokeFn !== undefined ? { invokeFn } : {})}
        {...(captureAudioControlsListenFn !== undefined
          ? { listenFn: captureAudioControlsListenFn }
          : {})}
      />
      {captureError !== null ? <CaptureErrorDisplay error={captureError} /> : null}
    </section>
  );
}

function resolveIsMacos(isMacos: boolean | undefined, detectMacosFn: () => boolean): boolean {
  return isMacos ?? detectMacosFn();
}

function DeviceSelectorPanelConnected(props: DeviceSelectorPanelProps) {
  const detectMacosFn = props.detectMacos ?? defaultDetectMacos;
  const audioDevices = useAudioDevices({
    ...(props.invokeFn !== undefined ? { invokeFn: props.invokeFn } : {}),
    ...(props.listenFn !== undefined
      ? { listenFn: props.listenFn as AudioDeviceEventListenFn }
      : {}),
  });
  const captureStatus = useCaptureStatus({
    ...(props.invokeFn !== undefined ? { invokeFn: props.invokeFn } : {}),
    ...(props.captureListenFn !== undefined ? { listenFn: props.captureListenFn } : {}),
  });

  const viewProps: DeviceSelectorPanelViewProps = {
    devices: props.devices ?? audioDevices.devices,
    selection: props.selection ?? audioDevices.selection,
    captureError: props.captureError !== undefined ? props.captureError : captureStatus.error,
    capturePhase: captureStatus.phase,
    isMacos: resolveIsMacos(props.isMacos, detectMacosFn),
    ...(props.onSelectionChange !== undefined
      ? { onSelectionChange: props.onSelectionChange }
      : {}),
    ...(props.invokeFn !== undefined ? { invokeFn: props.invokeFn } : {}),
    ...(props.listenFn !== undefined
      ? {
          captureAudioControlsListenFn: props.listenFn as CaptureAudioControlsEventListenFn,
        }
      : {}),
  };

  return <DeviceSelectorPanelView {...viewProps} />;
}

export function DeviceSelectorPanel(props: DeviceSelectorPanelProps = {}) {
  const detectMacosFn = props.detectMacos ?? defaultDetectMacos;
  const isInjected = props.devices !== undefined && props.selection !== undefined;
  if (isInjected) {
    const viewProps: DeviceSelectorPanelViewProps = {
      devices: props.devices,
      selection: props.selection,
      captureError: props.captureError ?? null,
      isMacos: resolveIsMacos(props.isMacos, detectMacosFn),
      ...(props.onSelectionChange !== undefined
        ? { onSelectionChange: props.onSelectionChange }
        : {}),
      ...(props.invokeFn !== undefined ? { invokeFn: props.invokeFn } : {}),
      ...(props.listenFn !== undefined
        ? {
            captureAudioControlsListenFn: props.listenFn as CaptureAudioControlsEventListenFn,
          }
        : {}),
    };
    return <DeviceSelectorPanelView {...viewProps} />;
  }
  return <DeviceSelectorPanelConnected {...props} />;
}
