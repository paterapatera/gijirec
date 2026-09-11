/**
 * Integration tests (design Integration 3–5):
 * save snapshot wiring through TranscriptEditorView + useSaveTranscript + mock invoke.
 */
import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { act, cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { createRef, type RefObject, useMemo } from "react";
import { Editor, Transforms } from "slate";
import { toJsonlRecords } from "../../domain/transcript/export";
import type { SaveTranscriptSessionResult } from "../../infrastructure/tauri/editorCommands";
import { asInjectableInvokeFn } from "../../infrastructure/tauri/injectableInvoke";
import { setupTestDom } from "../../test-setup";
import type { AiTranscriptEditorRef } from "../components/AiTranscriptEditor";
import {
  getHandwritingEditorForTest,
  type HandwritingEditorRef,
} from "../components/HandwritingEditor";
import { TranscriptEditorView } from "../components/TranscriptEditorView";
import { createMockListen } from "../components/transcriptEditorTestHelpers";
import { DEFAULT_EDITOR_SETTINGS, type EditorSettings } from "../hooks/editor-settings";
import type { TranscriptBlockAppended } from "../hooks/transcript-blocks";
import { BLOCK_APPENDED_EVENT } from "../hooks/transcript-blocks";
import { useEditorSettings } from "../hooks/useEditorSettings";
import { useSaveTranscript } from "../hooks/useSaveTranscript";
import { handleCommonTranscribeInvokeCommands } from "../testInvokeHelpers";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

type InvokeCall = { cmd: string; args?: Record<string, unknown> };

function makeBlockAppended(
  overrides: Partial<TranscriptBlockAppended["block"]> &
    Pick<TranscriptBlockAppended["block"], "block_id">,
): TranscriptBlockAppended {
  return {
    block: {
      sequence: 1,
      text: "AI 転写",
      start_timestamp_ms: 1_500,
      language: "ja",
      ...overrides,
    },
    timestamp_ms: 2_000,
  };
}

function createStatefulMockInvoke(
  options: {
    initial?: EditorSettings;
    pickResult?: string | null;
    saveHandler?: (
      args: Record<string, unknown> | undefined,
    ) => Promise<SaveTranscriptSessionResult>;
  } = {},
) {
  let persisted: EditorSettings = { ...(options.initial ?? DEFAULT_EDITOR_SETTINGS) };
  const calls: InvokeCall[] = [];
  const pickResult = options.pickResult ?? null;

  const invokeFn = async (cmd: string, args?: Record<string, unknown>) => {
    calls.push({ cmd, args });
    const transcribe = handleCommonTranscribeInvokeCommands(cmd);
    if (transcribe !== undefined) {
      return transcribe;
    }
    switch (cmd) {
      case "get_editor_settings":
        return { ...persisted };
      case "set_editor_settings":
        persisted = { ...persisted, ...args };
        return { ...persisted };
      case "pick_save_directory":
        if (pickResult !== null) {
          persisted = { ...persisted, save_directory: pickResult };
        }
        return pickResult;
      case "list_audio_devices":
        return { inputs: [], outputs: [] };
      case "get_device_selection":
        return { microphone_id: null, speaker_id: null };
      case "set_audio_device_ui_visible":
        return;
      case "save_transcript_session":
        if (options.saveHandler) {
          return options.saveHandler(args);
        }
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

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

async function waitForToolbarReady(getByTestId: (id: string) => HTMLElement): Promise<void> {
  await waitFor(() => {
    expect(getByTestId("pick-directory-button").hasAttribute("disabled")).toBe(false);
  });
}

function typeIntoHandwriting(ref: HandwritingEditorRef | null, text: string): void {
  act(() => {
    const editor = getHandwritingEditorForTest(ref);
    Transforms.select(editor, Editor.start(editor, [0]));
    Transforms.insertText(editor, text);
  });
}

interface SaveIntegrationHarnessProps {
  listenFn: ReturnType<typeof createMockListen>["listenFn"];
  invokeFn: ReturnType<typeof createStatefulMockInvoke>["invokeFn"];
  handwritingRef: RefObject<HandwritingEditorRef | null>;
  aiRef: RefObject<AiTranscriptEditorRef | null>;
}

function SaveIntegrationHarness({
  listenFn,
  invokeFn,
  handwritingRef,
  aiRef,
}: SaveIntegrationHarnessProps) {
  const settingsHook = useEditorSettings({ invokeFn });
  const handwritingPort = useMemo(
    () => ({
      getPlainText: () => handwritingRef.current?.getPlainText() ?? "",
    }),
    [handwritingRef],
  );
  const aiPort = useMemo(
    () => ({
      getBlocks: () => aiRef.current?.getBlocks() ?? [],
    }),
    [aiRef],
  );
  const { onSave, isSaving } = useSaveTranscript({
    handwritingEditor: handwritingPort,
    aiEditor: aiPort,
    settings: settingsHook.settings,
    sessionId: "integration-session",
    invokeFn,
    showSaveResultFn: () => {},
  });

  return (
    <TranscriptEditorView
      onSave={onSave}
      isSaving={isSaving}
      settings={settingsHook.settings}
      isLoading={settingsHook.isLoading}
      pickSaveDirectory={settingsHook.pickSaveDirectory}
      setExportJsonlEnabled={settingsHook.setExportJsonlEnabled}
      listenFn={listenFn}
      handwritingEditorRef={handwritingRef}
      aiTranscriptEditorRef={aiRef}
    />
  );
}

describe("Integration 3: invoke save captures editor markdown snapshots", () => {
  test("save invoke payload matches handwriting and AI editor content", async () => {
    const selectedPath = "C:\\Users\\test\\transcripts";
    const mock = createStatefulMockInvoke({ pickResult: selectedPath });
    const { listenFn, emit } = createMockListen();
    const handwritingRef = createRef<HandwritingEditorRef>();
    const aiRef = createRef<AiTranscriptEditorRef>();

    const { getByTestId } = render(
      <SaveIntegrationHarness
        listenFn={listenFn}
        invokeFn={mock.invokeFn}
        handwritingRef={handwritingRef}
        aiRef={aiRef}
      />,
    );

    await waitForToolbarReady(getByTestId);

    await act(async () => {
      fireEvent.click(getByTestId("pick-directory-button"));
    });

    typeIntoHandwriting(handwritingRef.current, "# 手動議事録");

    act(() => {
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({
          block_id: "save-block-1",
          sequence: 1,
          text: "一行目",
        }),
      );
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({
          block_id: "save-block-2",
          sequence: 2,
          text: "二行目",
        }),
      );
    });

    await waitFor(() => {
      expect(aiRef.current?.getBlocks()).toHaveLength(2);
    });

    await act(async () => {
      fireEvent.click(getByTestId("save-button"));
    });

    await waitFor(() => {
      const saveCall = mock.calls.find((call) => call.cmd === "save_transcript_session");
      expect(saveCall).toBeDefined();
      expect(saveCall?.args?.handwriting_markdown).toBe("# 手動議事録");
      expect(saveCall?.args?.ai_transcription_markdown).toBe("一行目\n二行目");
      expect(saveCall?.args?.session_id).toBe("integration-session");
    });
  });
});

describe("Integration 4: JSONL export toggle at save boundary", () => {
  test("omits ai_transcription_jsonl when export_jsonl_enabled is false", async () => {
    const selectedPath = "C:\\Users\\test\\transcripts";
    const mock = createStatefulMockInvoke({ pickResult: selectedPath });
    const { listenFn, emit } = createMockListen();
    const handwritingRef = createRef<HandwritingEditorRef>();
    const aiRef = createRef<AiTranscriptEditorRef>();

    const { getByTestId } = render(
      <SaveIntegrationHarness
        listenFn={listenFn}
        invokeFn={mock.invokeFn}
        handwritingRef={handwritingRef}
        aiRef={aiRef}
      />,
    );

    await waitForToolbarReady(getByTestId);

    await act(async () => {
      fireEvent.click(getByTestId("pick-directory-button"));
    });

    act(() => {
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({ block_id: "jsonl-off", sequence: 1, text: "plain" }),
      );
    });

    await waitFor(() => {
      expect(aiRef.current?.getBlocks()).toHaveLength(1);
    });

    await act(async () => {
      fireEvent.click(getByTestId("save-button"));
    });

    await waitFor(() => {
      const saveCall = mock.calls.find((call) => call.cmd === "save_transcript_session");
      expect(saveCall?.args?.ai_transcription_jsonl).toBeUndefined();
    });
  });

  test("includes ai_transcription_jsonl when export_jsonl_enabled is true", async () => {
    const selectedPath = "C:\\Users\\test\\transcripts";
    const mock = createStatefulMockInvoke({ pickResult: selectedPath });
    const { listenFn, emit } = createMockListen();
    const handwritingRef = createRef<HandwritingEditorRef>();
    const aiRef = createRef<AiTranscriptEditorRef>();

    const { getByTestId } = render(
      <SaveIntegrationHarness
        listenFn={listenFn}
        invokeFn={mock.invokeFn}
        handwritingRef={handwritingRef}
        aiRef={aiRef}
      />,
    );

    await waitForToolbarReady(getByTestId);

    await act(async () => {
      fireEvent.click(getByTestId("pick-directory-button"));
    });

    await act(async () => {
      fireEvent.click(getByTestId("export-jsonl-switch"));
    });

    act(() => {
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({ block_id: "jsonl-on", sequence: 1, text: "recorded" }),
      );
    });

    await waitFor(() => {
      expect(aiRef.current?.getBlocks()).toHaveLength(1);
    });

    await act(async () => {
      fireEvent.click(getByTestId("save-button"));
    });

    await waitFor(() => {
      const saveCall = mock.calls.find((call) => call.cmd === "save_transcript_session");
      expect(saveCall?.args?.ai_transcription_jsonl).toEqual(
        toJsonlRecords(aiRef.current?.getBlocks() ?? []),
      );
    });
  });
});

describe("Integration 5: blocks arriving during save excluded from export", () => {
  test("excludes blocks appended after save snapshot from invoke payload", async () => {
    const selectedPath = "C:\\Users\\test\\transcripts";
    const deferred = createDeferred<SaveTranscriptSessionResult>();
    const mock = createStatefulMockInvoke({
      pickResult: selectedPath,
      saveHandler: async () => deferred.promise,
    });
    const { listenFn, emit } = createMockListen();
    const handwritingRef = createRef<HandwritingEditorRef>();
    const aiRef = createRef<AiTranscriptEditorRef>();

    const { getByTestId } = render(
      <SaveIntegrationHarness
        listenFn={listenFn}
        invokeFn={mock.invokeFn}
        handwritingRef={handwritingRef}
        aiRef={aiRef}
      />,
    );

    await waitForToolbarReady(getByTestId);

    await act(async () => {
      fireEvent.click(getByTestId("pick-directory-button"));
    });

    act(() => {
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({ block_id: "during-save-1", sequence: 1, text: "snapshot" }),
      );
    });

    await waitFor(() => {
      expect(aiRef.current?.getBlocks()).toHaveLength(1);
    });

    await act(async () => {
      fireEvent.click(getByTestId("save-button"));
    });

    await waitFor(() => {
      expect(mock.calls.some((call) => call.cmd === "save_transcript_session")).toBe(true);
    });

    act(() => {
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({ block_id: "during-save-2", sequence: 2, text: "late" }),
      );
    });

    await waitFor(() => {
      expect(aiRef.current?.getBlocks()).toHaveLength(2);
    });

    const saveCall = mock.calls.find((call) => call.cmd === "save_transcript_session");
    expect(saveCall?.args?.ai_transcription_markdown).toBe("snapshot");
    expect(saveCall?.args?.ai_transcription_markdown).not.toContain("late");

    await act(async () => {
      deferred.resolve({
        success: true,
        output_directory: `${selectedPath}\\2026\\09\\06\\14_30_00`,
        files_written: [`${selectedPath}\\2026\\09\\06\\14_30_00\\ai-transcription.md`],
      });
      await deferred.promise;
    });
  });
});
