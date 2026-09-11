/**
 * Integration tests (design Integration 1–2):
 * upstream block-appended wiring and lock-then-append through composed AiTranscriptEditor.
 */
import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { createRef } from "react";
import { Transforms } from "slate";
import { lockSelection } from "../../application/transcript/lockManager";
import type { TranscriptBlockElement } from "../../domain/transcript/slateTypes";
import type { TranscriptBlockView } from "../../domain/transcript/types";
import type { SaveTranscriptSessionResult } from "../../infrastructure/tauri/editorCommands";
import { setupTestDom } from "../../test-setup";
import { DEFAULT_EDITOR_SETTINGS } from "../hooks/editor-settings";
import type { TranscriptBlockAppended } from "../hooks/transcript-blocks";
import { BLOCK_APPENDED_EVENT } from "../hooks/transcript-blocks";
import {
  AiTranscriptEditor,
  type AiTranscriptEditorRef,
  getAiTranscriptEditorForTest,
} from "./AiTranscriptEditor";
import { TranscriptEditorView } from "./TranscriptEditorView";
import { createMockListen, type MockTranscriptEditorListenFn } from "./transcriptEditorTestHelpers";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

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

function makeBlock(
  blockId: string,
  displayText: string,
  sequence = 1,
  startTimestampMs = 100,
): TranscriptBlockView {
  return {
    blockId,
    sequence,
    text: displayText,
    displayText,
    startTimestampMs,
    language: "ja",
  };
}

function appendBlock(ref: AiTranscriptEditorRef | null, block: TranscriptBlockView): void {
  act(() => {
    ref?.appendBlock(block);
  });
}

function lockTextRange(ref: AiTranscriptEditorRef | null, start: number, end: number): void {
  act(() => {
    const editor = getAiTranscriptEditorForTest(ref);
    Transforms.select(editor, {
      anchor: { path: [0, 0], offset: start },
      focus: { path: [0, 0], offset: end },
    });
    lockSelection(editor);
  });
}

const noopSave = async (): Promise<SaveTranscriptSessionResult | undefined> => undefined;

const defaultSettingsProps = {
  settings: DEFAULT_EDITOR_SETTINGS,
  isLoading: false,
  pickSaveDirectory: async () => {},
  setExportJsonlEnabled: async () => {},
};

describe("Integration 1: block-appended → AI editor end append", () => {
  test("appends multiple upstream blocks in sequence at document end via TranscriptEditorView", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const aiRef = createRef<AiTranscriptEditorRef>();
    const { container } = render(
      <TranscriptEditorView
        onSave={noopSave}
        isSaving={false}
        {...defaultSettingsProps}
        listenFn={listenFn}
        aiTranscriptEditorRef={aiRef}
      />,
    );

    await waitFor(() => {
      expect(listeners.has(BLOCK_APPENDED_EVENT)).toBe(true);
    });

    act(() => {
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({
          block_id: "block-1",
          sequence: 1,
          text: "最初のブロック",
        }),
      );
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({
          block_id: "block-2",
          sequence: 2,
          text: "末尾追記",
        }),
      );
    });

    await waitFor(() => {
      expect(aiRef.current?.getBlocks()).toHaveLength(2);
    });

    const blocks = container.querySelectorAll("[data-transcript-block]");
    expect(blocks).toHaveLength(2);
    expect(blocks[0]?.textContent).toBe("最初のブロック");
    expect(blocks[1]?.textContent).toBe("末尾追記");
    expect(aiRef.current?.getBlocks()[1]?.blockId).toBe("block-2");
  });

  test("flushes blocks received before AI editor ref attaches", async () => {
    const { listenFn, emit } = createMockListen();
    const gatedListenFn: MockTranscriptEditorListenFn = async (event, handler) => {
      const unlisten = await listenFn(event, handler);
      if (event === BLOCK_APPENDED_EVENT) {
        emit(
          BLOCK_APPENDED_EVENT,
          makeBlockAppended({
            block_id: "early-block",
            sequence: 1,
            text: "早期ブロック",
          }),
        );
      }
      return unlisten;
    };

    const aiRef = createRef<AiTranscriptEditorRef>();
    render(
      <TranscriptEditorView
        onSave={noopSave}
        isSaving={false}
        {...defaultSettingsProps}
        listenFn={gatedListenFn}
        aiTranscriptEditorRef={aiRef}
      />,
    );

    await waitFor(() => {
      expect(aiRef.current?.getBlocks()).toHaveLength(1);
    });
    expect(aiRef.current?.getBlocks()[0]?.displayText).toBe("早期ブロック");
  });
});

describe("Integration 2: lock then upstream append preserves locked text", () => {
  test("keeps locked text when appendBlock runs through composed AiTranscriptEditor", () => {
    const ref = createRef<AiTranscriptEditorRef>();
    const { container } = render(<AiTranscriptEditor ref={ref} />);

    appendBlock(ref.current, makeBlock("block-1", "hello world", 1));
    lockTextRange(ref.current, 0, 5);
    appendBlock(ref.current, makeBlock("block-2", "second block", 2));

    const editor = getAiTranscriptEditorForTest(ref.current);
    const firstBlock = editor.children[0] as TranscriptBlockElement;
    const lockedLeaf = firstBlock.children.find((child) => child.locked === true);
    expect(lockedLeaf?.text).toBe("hello");
    expect(ref.current?.getBlocks()).toHaveLength(2);
    expect(ref.current?.getBlocks()[0]?.displayText).toBe("hello world");

    const domBlocks = container.querySelectorAll("[data-transcript-block]");
    expect(domBlocks).toHaveLength(2);
    expect(domBlocks[1]?.textContent).toBe("second block");
  });

  test("keeps locked text when block-appended fires after user lock in TranscriptEditorView", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const aiRef = createRef<AiTranscriptEditorRef>();
    render(
      <TranscriptEditorView
        onSave={noopSave}
        isSaving={false}
        {...defaultSettingsProps}
        listenFn={listenFn}
        aiTranscriptEditorRef={aiRef}
      />,
    );

    await waitFor(() => {
      expect(listeners.has(BLOCK_APPENDED_EVENT)).toBe(true);
    });

    act(() => {
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({
          block_id: "block-lock-1",
          sequence: 1,
          text: "hello world",
        }),
      );
    });

    await waitFor(() => {
      expect(aiRef.current?.getBlocks()).toHaveLength(1);
    });

    lockTextRange(aiRef.current, 0, 5);

    act(() => {
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({
          block_id: "block-lock-2",
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
