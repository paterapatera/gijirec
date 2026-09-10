import { describe, expect, test } from "bun:test";
import type {
  CaptureAudioControls,
  CaptureAudioControlsChanged,
  CaptureAudioControlsState,
  CaptureAudioControlsUserError,
  IngestLevelChanged,
} from "../../presentation/hooks/capture-audio-controls-types";
import {
  CONTROLS_CHANGED_EVENT,
  DEFAULT_INGEST_GAIN,
  INGEST_LEVEL_EVENT,
} from "../../presentation/hooks/capture-audio-controls-types";
import { getCaptureAudioControls, setCaptureAudioControls } from "./captureAudioControlsCommands";

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

const sampleControls: CaptureAudioControls = {
  mic_ingest_enabled: false,
  manual_ingest_gain: 2.0,
  gain_user_adjusted: true,
};

const sampleState: CaptureAudioControlsState = {
  controls: sampleControls,
  ingest_level: {
    level_dbfs: -18.2,
    timestamp_ms: 1_700_000_000_000,
  },
};

const sampleControlsChanged: CaptureAudioControlsChanged = {
  controls: sampleControls,
  timestamp_ms: 1_700_000_000_001,
};

const sampleIngestLevelChanged: IngestLevelChanged = {
  level_dbfs: -17.5,
  timestamp_ms: 1_700_000_000_002,
};

const sampleUserError: CaptureAudioControlsUserError = {
  code: "INVALID_GAIN",
  message_ja: "ゲインの値が不正です",
  action_ja: "スライダーを中央付近に戻して再度お試しください",
};

describe("captureAudioControlsCommands", () => {
  test("contract event names match capture-audio-controls.md", () => {
    expect(CONTROLS_CHANGED_EVENT).toBe("capture-audio-controls://controls-changed");
    expect(INGEST_LEVEL_EVENT).toBe("capture-audio-controls://ingest-level");
    expect(sampleControlsChanged.controls.manual_ingest_gain).toBe(2.0);
    expect(sampleIngestLevelChanged.level_dbfs).toBe(-17.5);
    expect(sampleUserError.code).toBe("INVALID_GAIN");
  });

  test("getCaptureAudioControls invokes get_capture_audio_controls without args", async () => {
    const { invokeFn, calls } = createMockInvoke(sampleState);

    const actual = await getCaptureAudioControls({ invokeFn });

    expect(calls).toEqual([{ command: "get_capture_audio_controls" }]);
    expect(actual.controls.mic_ingest_enabled).toBe(false);
    expect(actual.controls.manual_ingest_gain).toBe(2.0);
    expect(actual.ingest_level?.level_dbfs).toBe(-18.2);
    expect(actual).toEqual(sampleState);
  });

  test("setCaptureAudioControls invokes set_capture_audio_controls with snake_case partial payload", async () => {
    const response: CaptureAudioControlsState = {
      controls: {
        mic_ingest_enabled: true,
        manual_ingest_gain: DEFAULT_INGEST_GAIN,
        gain_user_adjusted: false,
      },
      ingest_level: null,
    };
    const { invokeFn, calls } = createMockInvoke(response);

    const actual = await setCaptureAudioControls({ mic_ingest_enabled: true }, { invokeFn });

    expect(calls).toEqual([
      {
        command: "set_capture_audio_controls",
        args: { mic_ingest_enabled: true },
      },
    ]);
    expect(actual.controls.mic_ingest_enabled).toBe(true);
    expect(actual.ingest_level).toBeNull();
  });

  test("setCaptureAudioControls forwards manual_ingest_gain only", async () => {
    const { invokeFn, calls } = createMockInvoke(sampleState);

    await setCaptureAudioControls({ manual_ingest_gain: 3.5 }, { invokeFn });

    expect(calls).toEqual([
      {
        command: "set_capture_audio_controls",
        args: { manual_ingest_gain: 3.5 },
      },
    ]);
  });
});
