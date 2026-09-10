/** Contract types per `docs/contracts/capture-audio-controls.md`. */

export const CONTROLS_CHANGED_EVENT = "capture-audio-controls://controls-changed" as const;
export const INGEST_LEVEL_EVENT = "capture-audio-controls://ingest-level" as const;

export const MIN_INGEST_GAIN = 0.25;
export const MAX_INGEST_GAIN = 4.0;
export const DEFAULT_INGEST_GAIN = 1.25;

/** 転写 ingest ミックスへのマイク供給（OS ミュートではない） */
export interface CaptureAudioControls {
  /** true = マイクを ingest ミックスに含める。既定 true */
  mic_ingest_enabled: boolean;
  /** ingest 直前の線形ゲイン乗数。既定 1.25 */
  manual_ingest_gain: number;
  /** セッション中にユーザーがゲインを操作したら true */
  gain_user_adjusted: boolean;
}

/** メーター非活性時は get 応答で null */
export interface IngestLevelSnapshot {
  level_dbfs: number;
  timestamp_ms: number;
}

export interface CaptureAudioControlsState {
  controls: CaptureAudioControls;
  /** capturing かつ ingest へ供給可能なときのみ。それ以外は null */
  ingest_level: IngestLevelSnapshot | null;
}

export interface CaptureAudioControlsChanged {
  controls: CaptureAudioControls;
  timestamp_ms: number;
}

export interface IngestLevelChanged {
  level_dbfs: number;
  timestamp_ms: number;
}

export interface CaptureAudioControlsUserError {
  code: "INVALID_GAIN" | "INTERNAL";
  message_ja: string;
  action_ja: string;
}

export interface CaptureAudioControlsHookState {
  controls: CaptureAudioControls;
  ingest_level: IngestLevelSnapshot | null;
  /** true when capture phase is not `capturing` (req 1.6 / 3.5). */
  disabled: boolean;
}

export const INITIAL_CAPTURE_AUDIO_CONTROLS_HOOK_STATE: CaptureAudioControlsHookState = {
  controls: {
    mic_ingest_enabled: true,
    manual_ingest_gain: DEFAULT_INGEST_GAIN,
    gain_user_adjusted: false,
  },
  ingest_level: null,
  disabled: true,
};

type CaptureAudioControlsEventHandler = (event: {
  payload: CaptureAudioControlsChanged | IngestLevelChanged;
}) => void;

export type CaptureAudioControlsEventListenFn = (
  event: string,
  handler: CaptureAudioControlsEventHandler,
) => Promise<() => void>;
