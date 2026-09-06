import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { createRef } from "react";
import { Editor, Transforms } from "slate";
import { setupTestDom } from "../../test-setup";
import { DEFAULT_EDITOR_SETTINGS } from "../hooks/editor-settings";
import type { TranscribeUserError } from "../hooks/transcribe-status";
import { TRANSCRIBE_ERROR_EVENT } from "../hooks/transcribe-status";
import type { TranscriptBlockAppended } from "../hooks/transcript-blocks";
import { BLOCK_APPENDED_EVENT } from "../hooks/transcript-blocks";
import type { AiTranscriptEditorRef } from "./AiTranscriptEditor";
import { getHandwritingEditorForTest, type HandwritingEditorRef } from "./HandwritingEditor";
import { TranscriptEditorView } from "./TranscriptEditorView";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

type EventHandler = (event: { payload: unknown }) => void;

function createMockListen() {
  const listeners = new Map<string, EventHandler[]>();

  const listenFn = async (event: string, handler: EventHandler) => {
    const handlers = listeners.get(event) ?? [];
    handlers.push(handler);
    listeners.set(event, handlers);
    return () => {
      const list = listeners.get(event) ?? [];
      const index = list.indexOf(handler);
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

function typeIntoHandwriting(ref: HandwritingEditorRef | null, text: string): void {
  act(() => {
    const editor = getHandwritingEditorForTest(ref);
    Transforms.select(editor, Editor.start(editor, [0]));
    Transforms.insertText(editor, text);
  });
}

const noopSave = async () => {};

const defaultSettingsProps = {
  settings: DEFAULT_EDITOR_SETTINGS,
  isLoading: false,
  pickSaveDirectory: async () => {},
  setExportJsonlEnabled: async () => {},
};

describe("TranscriptEditorView", () => {
  test("renders handwriting-editor, ai-transcript-editor, and editor-toolbar", async () => {
    const { listenFn } = createMockListen();
    const { getByTestId } = render(
      <TranscriptEditorView
        onSave={noopSave}
        isSaving={false}
        {...defaultSettingsProps}
        listenFn={listenFn}
      />,
    );

    expect(getByTestId("handwriting-editor")).toBeTruthy();
    expect(getByTestId("ai-transcript-editor")).toBeTruthy();
    expect(getByTestId("editor-toolbar")).toBeTruthy();
  });

  test("includes Separator between editor panels", async () => {
    const { listenFn } = createMockListen();
    const { getByTestId } = render(
      <TranscriptEditorView
        onSave={noopSave}
        isSaving={false}
        {...defaultSettingsProps}
        listenFn={listenFn}
      />,
    );

    expect(getByTestId("transcript-editor-separator")).toBeTruthy();
  });

  test("hosts two independent Slate editors simultaneously", async () => {
    const { listenFn } = createMockListen();
    const handwritingRef = createRef<HandwritingEditorRef>();
    const aiRef = createRef<AiTranscriptEditorRef>();
    const { getByTestId } = render(
      <TranscriptEditorView
        onSave={noopSave}
        isSaving={false}
        {...defaultSettingsProps}
        listenFn={listenFn}
        handwritingEditorRef={handwritingRef}
        aiTranscriptEditorRef={aiRef}
      />,
    );

    const handwritingEl = getByTestId("handwriting-editor");
    const aiEl = getByTestId("ai-transcript-editor");

    expect(handwritingEl).not.toBe(aiEl);

    typeIntoHandwriting(handwritingRef.current, "手動メモ");

    expect(handwritingRef.current?.getPlainText()).toBe("手動メモ");
    expect(aiRef.current?.getBlocks()).toEqual([]);
  });

  test("appends upstream blocks to AI editor when block-appended event fires", async () => {
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
          text: "AI 転写結果",
        }),
      );
    });

    await waitFor(() => {
      const blockEl = container.querySelector("[data-transcript-block]");
      expect(blockEl?.textContent).toBe("AI 転写結果");
    });

    expect(aiRef.current?.getBlocks()).toHaveLength(1);
    expect(aiRef.current?.getBlocks()[0]?.displayText).toBe("AI 転写結果");
  });

  test("retains editor content and toolbar after transcribe error event", async () => {
    const { listenFn, emit, listeners } = createMockListen();
    const handwritingRef = createRef<HandwritingEditorRef>();
    const aiRef = createRef<AiTranscriptEditorRef>();
    const { getByTestId, container } = render(
      <TranscriptEditorView
        onSave={noopSave}
        isSaving={false}
        {...defaultSettingsProps}
        listenFn={listenFn}
        handwritingEditorRef={handwritingRef}
        aiTranscriptEditorRef={aiRef}
      />,
    );

    await waitFor(() => {
      expect(listeners.has(BLOCK_APPENDED_EVENT)).toBe(true);
      expect(listeners.has(TRANSCRIBE_ERROR_EVENT)).toBe(true);
    });

    typeIntoHandwriting(handwritingRef.current, "議事録本文");

    act(() => {
      emit(
        BLOCK_APPENDED_EVENT,
        makeBlockAppended({
          block_id: "block-err",
          sequence: 1,
          text: "転写保持",
        }),
      );
    });

    await waitFor(() => {
      expect(aiRef.current?.getBlocks()).toHaveLength(1);
    });

    const errorPayload: TranscribeUserError = {
      code: "INFERENCE_FAILED",
      message_ja: "推論に失敗しました",
      action_ja: "アプリを再起動してください",
      recoverable: true,
    };

    act(() => {
      emit(TRANSCRIBE_ERROR_EVENT, errorPayload);
    });

    await waitFor(() => {
      expect(handwritingRef.current?.getPlainText()).toBe("議事録本文");
      expect(aiRef.current?.getBlocks()[0]?.displayText).toBe("転写保持");
    });

    expect(getByTestId("editor-toolbar")).toBeTruthy();
    expect(container.querySelector("[data-transcript-block]")?.textContent).toBe("転写保持");
  });
});
