/**
 * E2E/UI tests (design E2E 1–5, Wave 29):
 * dual-editor input, lock-then-append, save toast, unset save dir, stable cursor on append.
 */
import { afterEach, beforeAll, beforeEach, describe, expect, mock, test } from "bun:test";
import { act, cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { createRef } from "react";
import { Editor, Range, Transforms } from "slate";
import { lockSelection } from "../application/transcript/lockManager";
import type { TranscriptBlockElement } from "../domain/transcript/slateTypes";
import type { SaveTranscriptSessionResult } from "../infrastructure/tauri/editorCommands";
import { asInjectableInvokeFn } from "../infrastructure/tauri/injectableInvoke";
import { setupTestDom } from "../test-setup";
import { App } from "./App";
import {
  type AiTranscriptEditorRef,
  getAiTranscriptEditorForTest,
} from "./components/AiTranscriptEditor";
import {
  getHandwritingEditorForTest,
  type HandwritingEditorRef,
} from "./components/HandwritingEditor";
import type { CaptureEventListenFn } from "./hooks/capture-status";
import type { EditorSettings } from "./hooks/editor-settings";
import { DEFAULT_EDITOR_SETTINGS } from "./hooks/editor-settings";
import type { TranscribeEventListenFn } from "./hooks/transcribe-status";
import type { TranscriptBlockAppended } from "./hooks/transcript-blocks";
import { BLOCK_APPENDED_EVENT } from "./hooks/transcript-blocks";
import { handleCommonTranscribeInvokeCommands } from "./testInvokeHelpers";

const mockToastSuccess = mock(() => {});
const mockToastError = mock(() => {});

mock.module("sonner", () => ({
  toast: {
    success: mockToastSuccess,
    error: mockToastError,
  },
}));

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

beforeEach(() => {
  mockToastSuccess.mockClear();
  mockToastError.mockClear();
});

type EventHandler = (event: { payload: unknown }) => void;
type InvokeCall = { cmd: string; args?: Record<string, unknown> };

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

function createStatefulMockInvoke(
  options: { initial?: EditorSettings; pickResult?: string | null } = {},
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

function makeBlockAppended(
  overrides: Partial<TranscriptBlockAppended["block"]> &
    Pick<TranscriptBlockAppended["block"], "block_id">,
): TranscriptBlockAppended {
  return {
    block: {
      sequence: 1,
      text: "転写テキスト",
      start_timestamp_ms: 500,
      language: "ja",
      ...overrides,
    },
    timestamp_ms: 1_000,
  };
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

function selectAiRange(ref: AiTranscriptEditorRef | null, start: number, end: number): void {
  act(() => {
    const editor = getAiTranscriptEditorForTest(ref);
    Transforms.select(editor, {
      anchor: { path: [0, 0], offset: start },
      focus: { path: [0, 0], offset: end },
    });
  });
}

function selectAiCursor(ref: AiTranscriptEditorRef | null, offset: number): void {
  selectAiRange(ref, offset, offset);
}

function getAiSelection(ref: AiTranscriptEditorRef | null): Range | null {
  const editor = getAiTranscriptEditorForTest(ref);
  return editor.selection ? { ...editor.selection } : null;
}

describe("E2E 1: dual editor simultaneous input (req 2.3)", () => {
  test("handwriting typing and upstream block append stay independent in App", async () => {
    const { listenFn, emit } = createMockListen();
    const mock = createStatefulMockInvoke();
    const handwritingRef = createRef<HandwritingEditorRef>();
    const aiRef = createRef<AiTranscriptEditorRef>();

    render(
      <App
        listenFn={listenFn}
        invokeFn={mock.invokeFn}
        handwritingEditorRef={handwritingRef}
        aiTranscriptEditorRef={aiRef}
      />,
    );

    await waitFor(() => {
      expect(handwritingRef.current).not.toBeNull();
      expect(aiRef.current).not.toBeNull();
    });

    typeIntoHandwriting(handwritingRef.current, "手動メモ");

    act(() => {
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({
          block_id: "dual-1",
          sequence: 1,
          text: "AI 転写一行",
        }),
      );
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({
          block_id: "dual-2",
          sequence: 2,
          text: "AI 二行目",
        }),
      );
    });

    await waitFor(() => {
      expect(aiRef.current?.getBlocks()).toHaveLength(2);
    });

    expect(handwritingRef.current?.getPlainText()).toBe("手動メモ");
    expect(aiRef.current?.getBlocks()[0]?.displayText).toBe("AI 転写一行");
    expect(aiRef.current?.getBlocks()[1]?.displayText).toBe("AI 二行目");
  });
});

describe("E2E 2: selection lock then append keeps locked (req 3.1)", () => {
  test("locked text survives upstream append through full App wiring", async () => {
    const { listenFn, emit } = createMockListen();
    const aiRef = createRef<AiTranscriptEditorRef>();

    render(
      <App
        listenFn={listenFn}
        invokeFn={createStatefulMockInvoke().invokeFn}
        aiTranscriptEditorRef={aiRef}
      />,
    );

    await waitFor(() => {
      expect(aiRef.current).not.toBeNull();
    });

    act(() => {
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({
          block_id: "lock-e2e-1",
          sequence: 1,
          text: "hello world",
        }),
      );
    });

    await waitFor(() => {
      expect(aiRef.current?.getBlocks()).toHaveLength(1);
    });

    selectAiRange(aiRef.current, 0, 5);
    act(() => {
      lockSelection(getAiTranscriptEditorForTest(aiRef.current));
    });

    act(() => {
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({
          block_id: "lock-e2e-2",
          sequence: 2,
          text: "追記ブロック",
        }),
      );
    });

    await waitFor(() => {
      expect(aiRef.current?.getBlocks()).toHaveLength(2);
    });

    const editor = getAiTranscriptEditorForTest(aiRef.current);
    const firstBlock = editor.children[0] as TranscriptBlockElement;
    const lockedLeaf = firstBlock.children.find((child) => child.locked === true);
    expect(lockedLeaf?.text).toBe("hello");
    expect(aiRef.current?.getBlocks()[1]?.displayText).toBe("追記ブロック");
  });
});

describe("E2E 3: save success shows path (req 7.6)", () => {
  test("successful save shows output_directory via toast", async () => {
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
      expect(mockToastSuccess).toHaveBeenCalledTimes(1);
    });

    const [, options] = mockToastSuccess.mock.calls[0]! as unknown as [
      string,
      { description?: string },
    ];
    expect(options.description).toBe(`${selectedPath}\\2026\\09\\06\\14_30_00`);
    expect(mockToastError).not.toHaveBeenCalled();
  });
});

describe("E2E 4: unset save dir notification (req 5.5)", () => {
  test("save without directory shows message_ja and action_ja via toast", async () => {
    const mock = createStatefulMockInvoke();
    const { listenFn } = createMockListen();
    const { getByTestId } = render(<App listenFn={listenFn} invokeFn={mock.invokeFn} />);

    await waitForToolbarReady(getByTestId);

    await act(async () => {
      fireEvent.click(getByTestId("save-button"));
    });

    await waitFor(() => {
      expect(mockToastError).toHaveBeenCalledTimes(1);
    });

    const [title, options] = mockToastError.mock.calls[0]! as unknown as [
      string,
      { description?: string },
    ];
    expect(title).toBe("保存先が設定されていません");
    expect(options.description).toBe("保存先フォルダを選択してください");
    expect(mockToastSuccess).not.toHaveBeenCalled();
  });
});

describe("E2E 5: high-frequency mock append keeps cursor (req 4.3)", () => {
  test("withStableSelection preserves mid-document cursor during rapid upstream appends", async () => {
    const { listenFn, emit } = createMockListen();
    const aiRef = createRef<AiTranscriptEditorRef>();

    render(
      <App
        listenFn={listenFn}
        invokeFn={createStatefulMockInvoke().invokeFn}
        aiTranscriptEditorRef={aiRef}
      />,
    );

    await waitFor(() => {
      expect(aiRef.current).not.toBeNull();
    });

    act(() => {
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({
          block_id: "cursor-base",
          sequence: 1,
          text: "aaaa bbbb cccc",
        }),
      );
    });

    await waitFor(() => {
      expect(aiRef.current?.getBlocks()).toHaveLength(1);
    });

    const cursorOffset = 5;
    selectAiCursor(aiRef.current, cursorOffset);

    const selectionBefore = getAiSelection(aiRef.current);
    expect(selectionBefore?.anchor.offset).toBe(cursorOffset);
    expect(selectionBefore?.focus.offset).toBe(cursorOffset);

    act(() => {
      for (let i = 2; i <= 21; i++) {
        emit(
          BLOCK_APPENDED_EVENT,
          makeBlockAppended({
            block_id: `cursor-burst-${i}`,
            sequence: i,
            text: `block-${i}`,
          }),
        );
      }
    });

    await waitFor(() => {
      expect(aiRef.current?.getBlocks()).toHaveLength(21);
    });

    const selectionAfter = getAiSelection(aiRef.current);
    expect(selectionAfter).not.toBeNull();
    expect(selectionAfter?.anchor.path).toEqual([0, 0]);
    expect(selectionAfter?.focus.path).toEqual([0, 0]);
    expect(selectionAfter?.anchor.offset).toBe(cursorOffset);
    expect(selectionAfter?.focus.offset).toBe(cursorOffset);
    expect(Range.isCollapsed(selectionAfter!)).toBe(true);
  });
});
