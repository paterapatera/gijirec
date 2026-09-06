import { describe, expect, test } from "bun:test";
import { createEditor, Editor, Transforms } from "slate";
import type { TranscriptBlockElement } from "../../../domain/transcript/slateTypes";
import { withAppendOnlyBlocks } from "./withAppendOnlyBlocks";

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

function createAppendOnlyEditor() {
  const editor = withAppendOnlyBlocks(createEditor());
  editor.children = [];
  return editor;
}

function appendBlockAtEnd(editor: Editor, block: TranscriptBlockElement) {
  Editor.withoutNormalizing(editor, () => {
    Transforms.insertNodes(editor, block, { at: [editor.children.length] });
  });
}

describe("withAppendOnlyBlocks", () => {
  test("allows inserting a transcript-block at document end", () => {
    const editor = createAppendOnlyEditor();
    const first = makeTranscriptBlock("block-1", "first");

    appendBlockAtEnd(editor, first);

    expect(editor.children).toHaveLength(1);
    expect((editor.children[0] as TranscriptBlockElement).blockId).toBe("block-1");
  });

  test("allows sequential end appends without mutating existing blocks", () => {
    const editor = createAppendOnlyEditor();
    const first = makeTranscriptBlock("block-1", "first");
    const second = makeTranscriptBlock("block-2", "second", 1);

    appendBlockAtEnd(editor, first);
    const firstRef = editor.children[0];
    appendBlockAtEnd(editor, second);

    expect(editor.children).toHaveLength(2);
    expect(editor.children[0]).toBe(firstRef);
    expect((editor.children[0] as TranscriptBlockElement).children[0]?.text).toBe("first");
    expect((editor.children[1] as TranscriptBlockElement).blockId).toBe("block-2");
  });

  test("rejects upstream removal of an existing transcript-block", () => {
    const editor = createAppendOnlyEditor();
    appendBlockAtEnd(editor, makeTranscriptBlock("block-1", "keep me"));

    Transforms.removeNodes(editor, { at: [0] });

    expect(editor.children).toHaveLength(1);
    expect((editor.children[0] as TranscriptBlockElement).children[0]?.text).toBe("keep me");
  });

  test("rejects upstream set_node on an existing transcript-block", () => {
    const editor = createAppendOnlyEditor();
    appendBlockAtEnd(editor, makeTranscriptBlock("block-1", "stable"));

    Transforms.setNodes(editor, { blockId: "mutated" }, { at: [0] });

    expect((editor.children[0] as TranscriptBlockElement).blockId).toBe("block-1");
  });

  test("rejects inserting a transcript-block anywhere except document end", () => {
    const editor = createAppendOnlyEditor();
    appendBlockAtEnd(editor, makeTranscriptBlock("block-1", "first"));
    appendBlockAtEnd(editor, makeTranscriptBlock("block-2", "second", 1));

    Transforms.insertNodes(editor, makeTranscriptBlock("block-middle", "middle", 2), {
      at: [0],
    });

    expect(editor.children).toHaveLength(2);
    expect(editor.children.map((node) => (node as TranscriptBlockElement).blockId)).toEqual([
      "block-1",
      "block-2",
    ]);
  });
});
