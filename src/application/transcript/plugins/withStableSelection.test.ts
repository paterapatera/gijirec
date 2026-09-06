import { describe, expect, test } from "bun:test";
import { createEditor, Editor, Transforms } from "slate";
import type { TranscriptBlockElement } from "../../../domain/transcript/slateTypes";
import { withAppendOnlyBlocks } from "./withAppendOnlyBlocks";
import { AI_TRANSCRIPT_SCROLL_STYLE, withStableSelection } from "./withStableSelection";

function makeTranscriptBlock(
  blockId: string,
  text: string,
  sequenceOffset = 0,
): TranscriptBlockElement {
  const sequence = sequenceOffset + 1;
  return {
    type: "transcript-block",
    blockId,
    sequence,
    upstreamText: text,
    startTimestampMs: 100 + sequenceOffset,
    language: "ja",
    children: [{ text }],
  };
}

function createStableEditor() {
  const editor = withStableSelection(withAppendOnlyBlocks(createEditor()));
  return editor;
}

function appendBlockAtEnd(editor: Editor, block: TranscriptBlockElement) {
  Editor.withoutNormalizing(editor, () => {
    Transforms.insertNodes(editor, block, { at: [editor.children.length] });
  });
}

describe("AI_TRANSCRIPT_SCROLL_STYLE", () => {
  test("exports overflow-anchor auto scroll container style for AiTranscriptEditor", () => {
    expect(AI_TRANSCRIPT_SCROLL_STYLE).toEqual({
      overflowY: "auto",
      overflowAnchor: "auto",
    });
  });
});

describe("withStableSelection", () => {
  test("preserves collapsed cursor in a non-end block after end append", () => {
    const editor = createStableEditor();
    editor.children = [
      makeTranscriptBlock("block-1", "hello world"),
      makeTranscriptBlock("block-2", "second", 1),
    ];

    const editSelection = {
      anchor: { path: [0, 0], offset: 3 },
      focus: { path: [0, 0], offset: 3 },
    };
    Transforms.select(editor, editSelection);

    appendBlockAtEnd(editor, makeTranscriptBlock("block-3", "third", 2));

    expect(editor.selection).toEqual(editSelection);
    expect(editor.stableSelectionRef.current).toEqual(editSelection);
  });

  test("preserves expanded selection in a non-end block after end append", () => {
    const editor = createStableEditor();
    editor.children = [
      makeTranscriptBlock("block-1", "hello world"),
      makeTranscriptBlock("block-2", "second", 1),
    ];

    const editSelection = {
      anchor: { path: [0, 0], offset: 0 },
      focus: { path: [0, 0], offset: 5 },
    };
    Transforms.select(editor, editSelection);

    appendBlockAtEnd(editor, makeTranscriptBlock("block-3", "third", 2));

    expect(editor.selection).toEqual(editSelection);
    expect(editor.stableSelectionRef.current).toEqual(editSelection);
  });

  test("updates stableSelectionRef when selection changes", () => {
    const editor = createStableEditor();
    editor.children = [makeTranscriptBlock("block-1", "hello")];

    const nextSelection = {
      anchor: { path: [0, 0], offset: 2 },
      focus: { path: [0, 0], offset: 4 },
    };
    Transforms.select(editor, nextSelection);

    expect(editor.stableSelectionRef.current).toEqual(nextSelection);
  });

  test("preserves cursor in last block when not at document end", () => {
    const editor = createStableEditor();
    editor.children = [makeTranscriptBlock("block-1", "hello")];

    const midSelection = {
      anchor: { path: [0, 0], offset: 2 },
      focus: { path: [0, 0], offset: 2 },
    };
    Transforms.select(editor, midSelection);

    appendBlockAtEnd(editor, makeTranscriptBlock("block-2", "world", 1));

    expect(editor.selection).toEqual(midSelection);
    expect(editor.stableSelectionRef.current).toEqual(midSelection);
  });

  test("does not force-restore when cursor is at document end", () => {
    const editor = createStableEditor();
    editor.children = [makeTranscriptBlock("block-1", "hello")];

    const endSelection = {
      anchor: { path: [0, 0], offset: 5 },
      focus: { path: [0, 0], offset: 5 },
    };
    Transforms.select(editor, endSelection);

    appendBlockAtEnd(editor, makeTranscriptBlock("block-2", "world", 1));

    expect(editor.children).toHaveLength(2);
    expect(editor.stableSelectionRef.current).toEqual(endSelection);
  });

  test("treats cursor at Editor.end as document end after locked split leaves", () => {
    const editor = createStableEditor();
    editor.children = [
      {
        ...makeTranscriptBlock("block-1", "hello"),
        children: [{ text: "hel", locked: true }, { text: "lo" }],
      },
    ];

    const endPoint = Editor.end(editor, [0]);
    Transforms.select(editor, endPoint);

    appendBlockAtEnd(editor, makeTranscriptBlock("block-2", "world", 1));

    expect(editor.children).toHaveLength(2);
    expect(editor.stableSelectionRef.current).toEqual({
      anchor: endPoint,
      focus: endPoint,
    });
  });
});
