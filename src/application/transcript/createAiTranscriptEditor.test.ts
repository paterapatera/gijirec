import { beforeAll, describe, expect, test } from "bun:test";
import { Editor, Element, Node, Transforms } from "slate";
import type { TranscriptBlockElement } from "../../domain/transcript/slateTypes";
import { setupTestDom } from "../../test-setup";
import { createAiTranscriptEditor } from "./createAiTranscriptEditor";

beforeAll(() => {
  setupTestDom();
});

function makeTranscriptBlock(blockId: string, text: string, sequence = 1): TranscriptBlockElement {
  return {
    type: "transcript-block",
    blockId,
    sequence,
    upstreamText: text,
    startTimestampMs: 100 + sequence,
    language: "ja",
    children: [{ text }],
  };
}

describe("createAiTranscriptEditor", () => {
  test("composes withAppendOnlyBlocks, withLockedRanges, and withStableSelection", () => {
    const editor = createAiTranscriptEditor();

    expect(typeof editor.applyUpstream).toBe("function");
    expect(editor.stableSelectionRef).toBeDefined();
    expect(editor.stableSelectionRef.current).toBeNull();
  });

  test("applyUpstream appends transcript-block at document end", () => {
    const editor = createAiTranscriptEditor();
    const block = makeTranscriptBlock("block-1", "hello");

    Editor.withoutNormalizing(editor, () => {
      editor.applyUpstream({
        type: "insert_node",
        path: [editor.children.length],
        node: block,
      });
    });

    expect(editor.children).toHaveLength(1);
    const inserted = editor.children[0];
    expect(Element.isElement(inserted) && inserted.type === "transcript-block").toBe(true);
    if (Element.isElement(inserted) && inserted.type === "transcript-block") {
      expect(inserted.blockId).toBe("block-1");
    }
  });

  test("applyUpstream rejects remove_text on locked leaves", () => {
    const editor = createAiTranscriptEditor();
    editor.children = [
      {
        ...makeTranscriptBlock("block-1", "hello"),
        children: [{ text: "hello", locked: true }],
      },
    ];

    const textBefore = Node.string(editor.children[0]!);
    editor.applyUpstream({
      type: "remove_text",
      path: [0, 0],
      offset: 0,
      text: "h",
    });

    expect(Node.string(editor.children[0]!)).toBe(textBefore);
  });

  test("user apply can insert text and locks the leaf", () => {
    const editor = createAiTranscriptEditor();
    editor.children = [makeTranscriptBlock("block-1", "hello")];
    Transforms.select(editor, { path: [0, 0], offset: 5 });

    Transforms.insertText(editor, "!");

    const leaf = Node.leaf(editor, [0, 0]);
    expect(leaf.text).toBe("hello!");
    expect(leaf.locked).toBe(true);
  });

  test("withAppendOnlyBlocks rejects non-end insert via applyUpstream", () => {
    const editor = createAiTranscriptEditor();
    editor.children = [makeTranscriptBlock("block-1", "first")];

    Editor.withoutNormalizing(editor, () => {
      editor.applyUpstream({
        type: "insert_node",
        path: [0],
        node: makeTranscriptBlock("block-0", "prepend"),
      });
    });

    expect(editor.children).toHaveLength(1);
    const only = editor.children[0];
    expect(Element.isElement(only) && only.type === "transcript-block" && only.blockId).toBe(
      "block-1",
    );
  });
});
