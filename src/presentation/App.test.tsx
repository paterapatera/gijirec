import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { act, cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import type { SaveTranscriptSessionResult } from "../infrastructure/tauri/editorCommands";
import { asInjectableInvokeFn } from "../infrastructure/tauri/injectableInvoke";
import { setupTestDom } from "../test-setup";
import { App } from "./App";
import type {
  CaptureEventListenFn,
  CapturePhaseChanged,
  CaptureUserError,
} from "./hooks/capture-status";
import { ERROR_EVENT, PHASE_CHANGED_EVENT } from "./hooks/capture-status";
import type { EditorSettings } from "./hooks/editor-settings";
import { DEFAULT_EDITOR_SETTINGS } from "./hooks/editor-settings";
import type {
  ModelDownloadProgress,
  TranscribeEventListenFn,
  TranscribePhaseChanged,
  TranscribeUserError,
} from "./hooks/transcribe-status";
import {
  MODEL_PROGRESS_EVENT,
  TRANSCRIBE_ERROR_EVENT,
  PHASE_CHANGED_EVENT as TRANSCRIBE_PHASE_CHANGED_EVENT,
} from "./hooks/transcribe-status";
import { BLOCK_APPENDED_EVENT } from "./hooks/transcript-blocks";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

type EventHandler = (event: { payload: unknown }) => void;

function createMockListen() {
  const listeners = new Map<string, EventHandler[]>();

  const listenFn: CaptureEventListenFn & TranscribeEventListenFn = async (event, handler) => {
    const handlers = listeners.get(event) ?? [];
    handlers.push(handler as EventHandler);
    listeners.set(event, handlers);
    return () => {
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

  return { listenFn, emit, listeners };
}

const defaultTranscribeSettingsResponse = {
  settings: { model_variant: "fp16" as const },
  local_availability: { q5_0: false, q8_0: false, fp16: true },
};

const defaultTranscribeStatusResponse = {
  phase: { phase: "ready" as const, timestamp_ms: 1 },
  model_progress: null,
};

const mockInvokeFn = asInjectableInvokeFn(async (cmd: string, _args?: Record<string, unknown>) => {
  if (cmd === "get_editor_settings") {
    return { save_directory: null, export_jsonl_enabled: false };
  }
  if (cmd === "get_transcribe_settings") {
    return defaultTranscribeSettingsResponse;
  }
  if (cmd === "get_transcribe_status") {
    return defaultTranscribeStatusResponse;
  }
  if (cmd === "list_audio_devices") {
    return { inputs: [], outputs: [] };
  }
  if (cmd === "get_device_selection") {
    return { microphone_id: null, speaker_id: null };
  }
  if (cmd === "set_audio_device_ui_visible") {
    return;
  }
  return {};
});

type InvokeCall = { cmd: string; args?: Record<string, unknown> };

function createStatefulMockInvoke(
  options: { initial?: EditorSettings; pickResult?: string | null } = {},
) {
  let persisted: EditorSettings = { ...(options.initial ?? DEFAULT_EDITOR_SETTINGS) };
  const calls: InvokeCall[] = [];
  const pickResult = options.pickResult ?? null;

  const invokeFn = async (cmd: string, args?: Record<string, unknown>) => {
    calls.push({ cmd, args });
    switch (cmd) {
      case "get_editor_settings":
        return { ...persisted };
      case "get_transcribe_settings":
        return defaultTranscribeSettingsResponse;
      case "get_transcribe_status":
        return defaultTranscribeStatusResponse;
      case "set_editor_settings":
        persisted = { ...persisted, ...args };
        return { ...persisted };
      case "pick_save_directory":
        return pickResult;
      case "list_audio_devices":
        return { inputs: [], outputs: [] };
      case "get_device_selection":
        return { microphone_id: null, speaker_id: null };
      case "set_audio_device_ui_visible":
        return;
      case "save_transcript_session":
        if (persisted.save_directory === null) {
          return {
            success: false,
            error: {
              code: "SAVE_DIRECTORY_NOT_SET",
              message_ja: "保存先が設定されていません",
              action_ja: "保存先フォルダを選択してください",
              recoverable: true,
            },
          } satisfies SaveTranscriptSessionResult;
        }
        return {
          success: true,
          output_directory: `${persisted.save_directory}\\2026\\09\\06\\14_30_00`,
          files_written: [`${persisted.save_directory}\\2026\\09\\06\\14_30_00\\handwriting.md`],
        } satisfies SaveTranscriptSessionResult;
      default:
        return {};
    }
  };

  return {
    invokeFn: asInjectableInvokeFn(invokeFn),
    calls,
    getPersisted: () => ({ ...persisted }),
  };
}

async function waitForToolbarReady(getByTestId: (id: string) => HTMLElement): Promise<void> {
  await waitFor(() => {
    expect(getByTestId("pick-directory-button").hasAttribute("disabled")).toBe(false);
  });
}

describe("App", () => {
  test("renders DeviceSelectorPanel adjacent to capture status (req 2.1)", async () => {
    const { listenFn } = createMockListen();
    const { getByTestId, getByLabelText } = render(
      <App listenFn={listenFn} invokeFn={mockInvokeFn} />,
    );

    await waitFor(() => {
      expect(getByTestId("capture-phase")).toBeTruthy();
      expect(getByLabelText("オーディオデバイス選択")).toBeTruthy();
    });

    const capturePhase = getByTestId("capture-phase");
    const devicePanel = getByLabelText("オーディオデバイス選択");
    expect(
      capturePhase.compareDocumentPosition(devicePanel) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(getByTestId("microphone-empty")).toBeTruthy();
    expect(getByTestId("speaker-empty")).toBeTruthy();
  });

  test("renders exactly one Toaster at root", async () => {
    const { listenFn } = createMockListen();
    render(<App listenFn={listenFn} invokeFn={mockInvokeFn} />);

    await waitFor(() => {
      expect(document.querySelectorAll('[aria-label="Notifications alt+T"]')).toHaveLength(1);
    });
  });

  test("save succeeds with updated save_directory after pick directory", async () => {
    const selectedPath = "C:\\Users\\test\\transcripts";
    const mock = createStatefulMockInvoke({ pickResult: selectedPath });
    const { listenFn } = createMockListen();
    const { getByTestId } = render(<App listenFn={listenFn} invokeFn={mock.invokeFn} />);

    await waitForToolbarReady(getByTestId);

    await act(async () => {
      fireEvent.click(getByTestId("pick-directory-button"));
    });

    await waitFor(() => {
      expect(mock.getPersisted().save_directory).toBe(selectedPath);
    });

    await act(async () => {
      fireEvent.click(getByTestId("save-button"));
    });

    await waitFor(() => {
      const saveCall = mock.calls.find((c) => c.cmd === "save_transcript_session");
      expect(saveCall).toBeDefined();
      expect(mock.getPersisted().save_directory).toBe(selectedPath);
    });

    const saveResults = mock.calls.filter((c) => c.cmd === "save_transcript_session");
    expect(saveResults.length).toBeGreaterThanOrEqual(1);
  });

  test("save request includes jsonl when export toggle updated via shared settings", async () => {
    const selectedPath = "C:\\Users\\test\\transcripts";
    const mock = createStatefulMockInvoke({ pickResult: selectedPath });
    const { listenFn } = createMockListen();
    const { getByTestId } = render(<App listenFn={listenFn} invokeFn={mock.invokeFn} />);

    await waitForToolbarReady(getByTestId);

    await act(async () => {
      fireEvent.click(getByTestId("pick-directory-button"));
    });

    await waitFor(() => {
      expect(mock.getPersisted().save_directory).toBe(selectedPath);
    });

    await act(async () => {
      fireEvent.click(getByTestId("export-jsonl-switch"));
    });

    await waitFor(() => {
      expect(getByTestId("export-jsonl-switch").getAttribute("data-state")).toBe("checked");
    });

    await act(async () => {
      fireEvent.click(getByTestId("save-button"));
    });

    await waitFor(() => {
      const saveCall = mock.calls.find((c) => c.cmd === "save_transcript_session");
      expect(saveCall?.args?.ai_transcription_jsonl).toBeDefined();
    });
  });

  test("renders transcript editor with dual Slate editors", async () => {
    const { listenFn } = createMockListen();
    const { getByTestId } = render(<App listenFn={listenFn} invokeFn={mockInvokeFn} />);

    await waitFor(() => {
      expect(getByTestId("transcript-editor-view")).toBeTruthy();
      expect(getByTestId("handwriting-editor")).toBeTruthy();
      expect(getByTestId("ai-transcript-editor")).toBeTruthy();
    });
  });

  test("subscribes to block-appended events via shared listenFn", async () => {
    const { listenFn, listeners } = createMockListen();
    render(<App listenFn={listenFn} invokeFn={mockInvokeFn} />);

    await waitFor(() => {
      expect(listeners.has(BLOCK_APPENDED_EVENT)).toBe(true);
    });
  });

  test("shows capturing phase after phase-changed event", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { getByTestId } = render(<App listenFn={listenFn} invokeFn={mockInvokeFn} />);

    await waitFor(() => {
      expect(listeners.has(PHASE_CHANGED_EVENT)).toBe(true);
    });

    const payload: CapturePhaseChanged = {
      phase: "capturing",
      timestamp_ms: 1_234_567_890,
    };
    act(() => {
      emit(PHASE_CHANGED_EVENT, payload);
    });

    await waitFor(() => {
      expect(getByTestId("capture-phase").textContent).toBe("capturing");
    });
  });

  test("shows message_ja and prominent action_ja for permission denied", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { getByTestId, container } = render(<App listenFn={listenFn} invokeFn={mockInvokeFn} />);

    await waitFor(() => {
      expect(listeners.has(ERROR_EVENT)).toBe(true);
    });

    const payload: CaptureUserError = {
      code: "MIC_PERMISSION_DENIED",
      message_ja: "マイクへのアクセスが拒否されました",
      action_ja: "設定 → プライバシー → マイクで gijirec を許可してください",
      recoverable: true,
    };
    act(() => {
      emit(ERROR_EVENT, payload);
    });

    await waitFor(() => {
      expect(getByTestId("error-message").textContent).toBe(payload.message_ja);
    });

    const action = getByTestId("error-action");
    expect(action.textContent).toBe(payload.action_ja);
    expect(action.className).toContain("error-action");

    expect(container.textContent).not.toContain("MIC_PERMISSION_DENIED");
  });

  // whisper-transcribe status UI tests (Task 7.2 / E2E 1-4)
  // E2E 1: アプリ起動 → モデル未取得時 loading_model 表示と進捗バー
  test("shows loading_model and model progress bar during model download (E2E 1)", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { getByTestId } = render(<App listenFn={listenFn} invokeFn={mockInvokeFn} />);

    await waitFor(() => {
      expect(listeners.has(TRANSCRIBE_PHASE_CHANGED_EVENT)).toBe(true);
      expect(listeners.has(MODEL_PROGRESS_EVENT)).toBe(true);
    });

    act(() => {
      emit(TRANSCRIBE_PHASE_CHANGED_EVENT, {
        phase: "loading_model",
        timestamp_ms: 100,
      } satisfies TranscribePhaseChanged);
      emit(MODEL_PROGRESS_EVENT, {
        bytes_downloaded: 250_000,
        bytes_total: 1_000_000,
        percent: 25,
        status: "downloading",
      } satisfies ModelDownloadProgress);
    });

    await waitFor(() => {
      expect(getByTestId("transcribe-phase").textContent).toBe("loading_model");
    });

    expect(getByTestId("model-progress-status").textContent).toContain("25%");
    const progressBar = getByTestId("model-progress-bar") as HTMLProgressElement;
    expect(progressBar.value).toBe(25);
  });

  // E2E 2: キャプチャ中 → transcribing フェーズ表示（capture capturing と連動）
  test("shows transcribing phase when capture becomes capturing and transcribe starts (E2E 2)", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { getByTestId } = render(<App listenFn={listenFn} invokeFn={mockInvokeFn} />);

    await waitFor(() => {
      expect(listeners.has(PHASE_CHANGED_EVENT)).toBe(true);
      expect(listeners.has(TRANSCRIBE_PHASE_CHANGED_EVENT)).toBe(true);
    });

    act(() => {
      emit(PHASE_CHANGED_EVENT, {
        phase: "capturing",
        timestamp_ms: 100,
      } satisfies CapturePhaseChanged);
      emit(TRANSCRIBE_PHASE_CHANGED_EVENT, {
        phase: "transcribing",
        timestamp_ms: 200,
      } satisfies TranscribePhaseChanged);
    });

    await waitFor(() => {
      expect(getByTestId("capture-phase").textContent).toBe("capturing");
      expect(getByTestId("transcribe-phase").textContent).toBe("transcribing");
    });
  });

  // E2E 3: ウィンドウ閉鎖 / アプリ終了時のクリーンアップ・リスナー解除
  test("unsubscribes transcribe listeners cleanly on unmount (E2E 3)", async () => {
    const { listenFn, listeners } = createMockListen();
    const { unmount } = render(<App listenFn={listenFn} invokeFn={mockInvokeFn} />);

    await waitFor(() => {
      expect(listeners.get(TRANSCRIBE_PHASE_CHANGED_EVENT)?.length).toBe(1);
      expect(listeners.get(MODEL_PROGRESS_EVENT)?.length).toBe(1);
      expect(listeners.get(TRANSCRIBE_ERROR_EVENT)?.length).toBe(1);
      expect(listeners.has(BLOCK_APPENDED_EVENT)).toBe(true);
    });

    unmount();

    expect(listeners.get(TRANSCRIBE_PHASE_CHANGED_EVENT)?.length).toBe(0);
    expect(listeners.get(MODEL_PROGRESS_EVENT)?.length).toBe(0);
    expect(listeners.get(TRANSCRIBE_ERROR_EVENT)?.length).toBe(0);
    expect(listeners.get(BLOCK_APPENDED_EVENT)?.length).toBe(0);
  });

  // E2E 4: モデル破損ファイル → エラーメッセージと action_ja 表示
  test("shows transcribe error message and action_ja when transcribe error event fires (E2E 4)", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const { getByTestId, container } = render(<App listenFn={listenFn} invokeFn={mockInvokeFn} />);

    await waitFor(() => {
      expect(listeners.has(TRANSCRIBE_ERROR_EVENT)).toBe(true);
    });

    const payload: TranscribeUserError = {
      code: "MODEL_CORRUPT",
      message_ja: "音声認識モデルファイルが破損しています",
      action_ja: "アプリを再起動してモデルを再取得してください",
      recoverable: true,
    };

    act(() => {
      emit(TRANSCRIBE_ERROR_EVENT, payload);
    });

    await waitFor(() => {
      expect(getByTestId("transcribe-error-message").textContent).toBe(payload.message_ja);
      expect(getByTestId("transcribe-error-action").textContent).toBe(payload.action_ja);
    });

    expect(container.textContent).not.toContain("MODEL_CORRUPT");
  });

  test("renders model variant selector with three choices (req 1.1, 1.2)", async () => {
    const { listenFn } = createMockListen();
    const { getByTestId, getByText } = render(<App listenFn={listenFn} invokeFn={mockInvokeFn} />);

    await waitFor(() => {
      const select = getByTestId("model-variant-select") as HTMLSelectElement;
      expect(select.options.length).toBe(3);
      expect(select.value).toBe("fp16");
      expect(getByText(/現在: FP16/)).toBeTruthy();
    });
  });

  test("syncs transcribe phase from get_transcribe_status on mount (req 5.1)", async () => {
    const invokeFn = asInjectableInvokeFn(async (cmd: string) => {
      if (cmd === "get_transcribe_status") {
        return {
          phase: { phase: "loading_model", timestamp_ms: 42 },
          model_progress: null,
        };
      }
      return mockInvokeFn(cmd);
    });
    const { listenFn } = createMockListen();
    const { getByTestId } = render(<App listenFn={listenFn} invokeFn={invokeFn} />);

    await waitFor(() => {
      expect(getByTestId("transcribe-phase").textContent).toBe("loading_model");
    });
  });

  test("disables model variant select while loading_model (req 3.2)", async () => {
    const invokeFn = asInjectableInvokeFn(async (cmd: string) => {
      if (cmd === "get_transcribe_status") {
        return {
          phase: { phase: "loading_model", timestamp_ms: 42 },
          model_progress: null,
        };
      }
      return mockInvokeFn(cmd);
    });
    const { listenFn } = createMockListen();
    const { getByTestId } = render(<App listenFn={listenFn} invokeFn={invokeFn} />);

    await waitFor(() => {
      const select = getByTestId("model-variant-select") as HTMLSelectElement;
      expect(select.disabled).toBe(true);
    });
  });
});
