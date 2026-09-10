import type { invoke } from "@tauri-apps/api/core";
import type { InputEvent } from "react";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { setCaptureAudioControls } from "../../infrastructure/tauri/captureAudioControlsCommands";
import type {
  CaptureAudioControls,
  CaptureAudioControlsEventListenFn,
  IngestLevelSnapshot,
} from "../hooks/capture-audio-controls-types";
import { MAX_INGEST_GAIN, MIN_INGEST_GAIN } from "../hooks/capture-audio-controls-types";
import type { CapturePhaseChanged } from "../hooks/capture-status";
import {
  type UseCaptureAudioControlsOptions,
  useCaptureAudioControls,
} from "../hooks/useCaptureAudioControls";

type CapturePhase = CapturePhaseChanged["phase"];

const GAIN_STEP = 0.05;
const GAIN_MIN_HINT = "ゲインは下限（0.25）に達しました";
const GAIN_MAX_HINT = "ゲインは上限（4.0）に達しました";
const INACTIVE_METER_TEXT = "—";

interface CaptureAudioControlsRowInjectedProps {
  readonly controls: CaptureAudioControls;
  readonly ingest_level: IngestLevelSnapshot | null;
  readonly disabled: boolean;
}

interface CaptureAudioControlsRowRuntimeProps {
  readonly invokeFn?: typeof invoke;
  readonly listenFn?: CaptureAudioControlsEventListenFn;
  readonly capturePhase?: CapturePhase;
}

export type CaptureAudioControlsRowProps = Partial<CaptureAudioControlsRowInjectedProps> &
  CaptureAudioControlsRowRuntimeProps;

interface CaptureAudioControlsRowViewProps extends CaptureAudioControlsRowInjectedProps {
  readonly invokeFn?: typeof invoke;
}

function formatDbfs(levelDbfs: number): string {
  const formatted = levelDbfs.toFixed(1);
  const withMinus = formatted.startsWith("-") ? `−${formatted.slice(1)}` : formatted;
  return `${withMinus} dBFS`;
}

function formatGain(gain: number): string {
  return gain.toFixed(2);
}

function resolveMeterText(disabled: boolean, ingestLevel: IngestLevelSnapshot | null): string {
  if (disabled || ingestLevel === null) {
    return INACTIVE_METER_TEXT;
  }
  return formatDbfs(ingestLevel.level_dbfs);
}

function resolveGainLimitHint(gain: number, disabled: boolean): string | null {
  if (disabled) {
    return null;
  }
  if (gain <= MIN_INGEST_GAIN) {
    return GAIN_MIN_HINT;
  }
  if (gain >= MAX_INGEST_GAIN) {
    return GAIN_MAX_HINT;
  }
  return null;
}

function CaptureAudioControlsRowView({
  controls,
  ingest_level,
  disabled,
  invokeFn,
}: CaptureAudioControlsRowViewProps) {
  const meterText = resolveMeterText(disabled, ingest_level);
  const gainText = disabled ? INACTIVE_METER_TEXT : formatGain(controls.manual_ingest_gain);
  const gainHint = resolveGainLimitHint(controls.manual_ingest_gain, disabled);

  const applyControls = (patch: Partial<CaptureAudioControls>): void => {
    void setCaptureAudioControls(patch, invokeFn !== undefined ? { invokeFn } : {});
  };

  const handleMicToggle = (checked: boolean): void => {
    applyControls({ mic_ingest_enabled: checked });
  };

  const handleGainChange = (event: InputEvent<HTMLInputElement>): void => {
    const gain = Number.parseFloat(event.currentTarget.value);
    applyControls({ manual_ingest_gain: gain });
  };

  return (
    <section
      className="capture-audio-controls-row"
      data-testid="capture-audio-controls-row"
      aria-label="キャプチャ音声制御"
    >
      <div className="capture-audio-field capture-audio-mic-field">
        <Label htmlFor="mic-ingest-switch">マイク</Label>
        <Switch
          id="mic-ingest-switch"
          data-testid="mic-ingest-switch"
          checked={controls.mic_ingest_enabled}
          disabled={disabled}
          onCheckedChange={handleMicToggle}
        />
      </div>
      <div className="capture-audio-field capture-audio-meter-field">
        <span className="status-label">レベル</span>
        <span
          className="capture-audio-meter"
          data-testid="ingest-level-meter"
          role="status"
          aria-label={
            meterText === INACTIVE_METER_TEXT ? "レベルメーター非活性" : "ingest 直前レベル"
          }
        >
          {meterText}
        </span>
      </div>
      <div className="capture-audio-field capture-audio-gain-field">
        <Label htmlFor="ingest-gain-slider">ゲイン</Label>
        <input
          id="ingest-gain-slider"
          type="range"
          className="capture-audio-gain-slider"
          data-testid="ingest-gain-slider"
          min={MIN_INGEST_GAIN}
          max={MAX_INGEST_GAIN}
          step={GAIN_STEP}
          value={controls.manual_ingest_gain}
          disabled={disabled}
          onInput={handleGainChange}
        />
        <span
          className="capture-audio-gain-value"
          data-testid="ingest-gain-value"
          aria-hidden="true"
        >
          {gainText}
        </span>
        <div className="capture-audio-gain-hint" data-testid="gain-limit-hint" aria-live="polite">
          {gainHint ?? ""}
        </div>
      </div>
    </section>
  );
}

function CaptureAudioControlsRowConnected(props: CaptureAudioControlsRowRuntimeProps) {
  const hookOptions: UseCaptureAudioControlsOptions = {
    ...(props.invokeFn !== undefined ? { invokeFn: props.invokeFn } : {}),
    ...(props.listenFn !== undefined ? { listenFn: props.listenFn } : {}),
    ...(props.capturePhase !== undefined ? { capturePhase: props.capturePhase } : {}),
  };
  const { controls, ingest_level, disabled } = useCaptureAudioControls(hookOptions);

  const viewProps: CaptureAudioControlsRowViewProps = {
    controls,
    ingest_level,
    disabled,
    ...(props.invokeFn !== undefined ? { invokeFn: props.invokeFn } : {}),
  };

  return <CaptureAudioControlsRowView {...viewProps} />;
}

function isInjectedProps(
  props: CaptureAudioControlsRowProps,
): props is CaptureAudioControlsRowInjectedProps & CaptureAudioControlsRowRuntimeProps {
  return props.controls !== undefined && props.disabled !== undefined && "ingest_level" in props;
}

export function CaptureAudioControlsRow(props: CaptureAudioControlsRowProps = {}) {
  if (isInjectedProps(props)) {
    const viewProps: CaptureAudioControlsRowViewProps = {
      controls: props.controls,
      ingest_level: props.ingest_level,
      disabled: props.disabled,
      ...(props.invokeFn !== undefined ? { invokeFn: props.invokeFn } : {}),
    };
    return <CaptureAudioControlsRowView {...viewProps} />;
  }
  return <CaptureAudioControlsRowConnected {...props} />;
}
