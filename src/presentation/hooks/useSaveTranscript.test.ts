import { afterEach, beforeAll, describe, expect, mock, test } from "bun:test";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import type {
  AiTranscriptEditorSnapshotPort,
  HandwritingEditorSnapshotPort,
} from "../../application/transcript/saveOrchestrator";
import type { TranscriptBlockView } from "../../domain/transcript/types";
import type { SaveTranscriptSessionResult } from "../../infrastructure/tauri/editorCommands";
import { asInjectableInvokeFn } from "../../infrastructure/tauri/injectableInvoke";
import { setupTestDom } from "../../test-setup";
import type { EditorSettings } from "./editor-settings";
import { useSaveTranscript } from "./useSaveTranscript";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

const TEST_SETTINGS: EditorSettings = {
  save_directory: "C:\\Users\\test\\Documents\\gijirec",
  export_jsonl_enabled: false,
};

const SAMPLE_BLOCKS: TranscriptBlockView[] = [
  {
    blockId: "block-1",
    sequence: 1,
    text: "upstream",
    displayText: "display",
    startTimestampMs: 1000,
    language: "ja",
  },
];

function createMockEditors(overrides?: { plainText?: string; blocks?: TranscriptBlockView[] }) {
  const plainText = overrides?.plainText ?? "手動議事録";
  const blocks = overrides?.blocks ?? SAMPLE_BLOCKS;

  const handwritingEditor: HandwritingEditorSnapshotPort = {
    getPlainText: () => plainText,
  };

  const aiEditor: AiTranscriptEditorSnapshotPort = {
    getBlocks: () => blocks,
  };

  return { handwritingEditor, aiEditor, plainText, blocks };
}

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

describe("useSaveTranscript", () => {
  test("calls showSaveResult with output_directory on success", async () => {
    const { handwritingEditor, aiEditor } = createMockEditors();
    const showSaveResultFn = mock((_result: SaveTranscriptSessionResult) => {});
    const outputDirectory = "C:\\Users\\test\\Documents\\gijirec\\2026\\09\\06\\14_30_00";

    const invokeFn = mock(
      async () =>
        ({
          success: true,
          output_directory: outputDirectory,
          files_written: [
            `${outputDirectory}\\handwriting.md`,
            `${outputDirectory}\\ai-transcription.md`,
          ],
        }) satisfies SaveTranscriptSessionResult,
    );

    const { result } = renderHook(() =>
      useSaveTranscript({
        handwritingEditor,
        aiEditor,
        settings: TEST_SETTINGS,
        sessionId: "session-success",
        invokeFn: asInjectableInvokeFn(invokeFn),
        showSaveResultFn,
      }),
    );

    await act(async () => {
      await result.current.onSave();
    });

    expect(showSaveResultFn).toHaveBeenCalledTimes(1);
    const [saveResult] = showSaveResultFn.mock.calls[0]! as [SaveTranscriptSessionResult];
    expect(saveResult.success).toBe(true);
    expect(saveResult.output_directory).toBe(outputDirectory);
    expect(invokeFn).toHaveBeenCalledTimes(1);
  });

  test("shows SAVE_DIRECTORY_NOT_SET error without clearing editor snapshots", async () => {
    const { handwritingEditor, aiEditor, plainText, blocks } = createMockEditors();
    const showSaveResultFn = mock((_result: SaveTranscriptSessionResult) => {});

    const invokeFn = mock(
      async () =>
        ({
          success: false,
          error: {
            code: "SAVE_DIRECTORY_NOT_SET",
            message_ja: "保存先フォルダが設定されていません",
            action_ja: "設定画面で保存先フォルダを選択してください",
            recoverable: true,
          },
        }) satisfies SaveTranscriptSessionResult,
    );

    const { result } = renderHook(() =>
      useSaveTranscript({
        handwritingEditor,
        aiEditor,
        settings: { ...TEST_SETTINGS, save_directory: null },
        sessionId: "session-not-set",
        invokeFn: asInjectableInvokeFn(invokeFn),
        showSaveResultFn,
      }),
    );

    await act(async () => {
      await result.current.onSave();
    });

    expect(showSaveResultFn).toHaveBeenCalledTimes(1);
    const [saveResult] = showSaveResultFn.mock.calls[0]! as [SaveTranscriptSessionResult];
    expect(saveResult.success).toBe(false);
    expect(saveResult.error?.code).toBe("SAVE_DIRECTORY_NOT_SET");

    expect(handwritingEditor.getPlainText()).toBe(plainText);
    expect(aiEditor.getBlocks()).toEqual(blocks);
  });

  test("passes partial failure result with files_failed to showSaveResult", async () => {
    const { handwritingEditor, aiEditor } = createMockEditors();
    const showSaveResultFn = mock((_result: SaveTranscriptSessionResult) => {});

    const partialResult: SaveTranscriptSessionResult = {
      success: false,
      output_directory: "C:\\Users\\test\\Documents\\gijirec\\2026\\09\\06\\14_30_01",
      files_written: [
        "C:\\Users\\test\\Documents\\gijirec\\2026\\09\\06\\14_30_01\\handwriting.md",
      ],
      files_failed: [
        {
          path: "C:\\Users\\test\\Documents\\gijirec\\2026\\09\\06\\14_30_01\\ai-transcription.jsonl",
          reason_ja: "書き込みに失敗しました",
        },
      ],
      error: {
        code: "SAVE_PARTIAL_FAILURE",
        message_ja: "一部のファイルの保存に失敗しました",
        action_ja: "失敗したファイルを確認して再試行してください",
        recoverable: true,
      },
    };

    const invokeFn = mock(async () => partialResult);

    const { result } = renderHook(() =>
      useSaveTranscript({
        handwritingEditor,
        aiEditor,
        settings: { ...TEST_SETTINGS, export_jsonl_enabled: true },
        sessionId: "session-partial",
        invokeFn: asInjectableInvokeFn(invokeFn),
        showSaveResultFn,
      }),
    );

    await act(async () => {
      await result.current.onSave();
    });

    expect(showSaveResultFn).toHaveBeenCalledTimes(1);
    const [saveResult] = showSaveResultFn.mock.calls[0]! as [SaveTranscriptSessionResult];
    expect(saveResult.success).toBe(false);
    expect(saveResult.files_failed).toEqual(partialResult.files_failed);
    expect(saveResult.error?.code).toBe("SAVE_PARTIAL_FAILURE");
  });

  test("sets isSaving during in-flight save and deduplicates concurrent saves", async () => {
    const { handwritingEditor, aiEditor } = createMockEditors();
    const showSaveResultFn = mock((_result: SaveTranscriptSessionResult) => {});
    const deferred = createDeferred<SaveTranscriptSessionResult>();

    const invokeFn = mock(async () => deferred.promise);

    const { result } = renderHook(() =>
      useSaveTranscript({
        handwritingEditor,
        aiEditor,
        settings: TEST_SETTINGS,
        sessionId: "session-concurrent",
        invokeFn: asInjectableInvokeFn(invokeFn),
        showSaveResultFn,
      }),
    );

    let firstSavePromise!: Promise<SaveTranscriptSessionResult | undefined>;
    act(() => {
      firstSavePromise = result.current.onSave();
    });

    await waitFor(() => {
      expect(result.current.isSaving).toBe(true);
    });

    let secondSavePromise!: Promise<SaveTranscriptSessionResult | undefined>;
    act(() => {
      secondSavePromise = result.current.onSave();
    });

    expect(invokeFn).toHaveBeenCalledTimes(1);

    const successResult: SaveTranscriptSessionResult = {
      success: true,
      output_directory: "C:\\Users\\test\\out",
    };

    await act(async () => {
      deferred.resolve(successResult);
      await Promise.all([firstSavePromise, secondSavePromise]);
    });

    expect(showSaveResultFn).toHaveBeenCalledTimes(1);
    expect(result.current.isSaving).toBe(false);
  });

  test("shows INTERNAL error when invoke rejects instead of failing silently", async () => {
    const { handwritingEditor, aiEditor } = createMockEditors();
    const showSaveResultFn = mock((_result: SaveTranscriptSessionResult) => {});
    const invokeFn = mock(async () => {
      throw new Error("command save_transcript_session not allowed");
    });

    const { result } = renderHook(() =>
      useSaveTranscript({
        handwritingEditor,
        aiEditor,
        settings: TEST_SETTINGS,
        sessionId: "session-invoke-reject",
        invokeFn: asInjectableInvokeFn(invokeFn),
        showSaveResultFn,
      }),
    );

    let saveResult: SaveTranscriptSessionResult | undefined;
    await act(async () => {
      saveResult = await result.current.onSave();
    });

    expect(saveResult?.success).toBe(false);
    expect(saveResult?.error?.code).toBe("INTERNAL");
    expect(showSaveResultFn).toHaveBeenCalledTimes(1);
    const [shown] = showSaveResultFn.mock.calls[0]! as [SaveTranscriptSessionResult];
    expect(shown.error?.code).toBe("INTERNAL");
    expect(shown.error?.action_ja).toContain("保存先");
    expect(result.current.isSaving).toBe(false);
  });
});
