import { describe, expect, test } from "bun:test";
import { createEditor, Transforms } from "slate";
import type { TranscriptBlockElement } from "../../domain/transcript/slateTypes";
import { lockAtPath, lockSelection } from "./lockManager";
import { withLockedRanges } from "./plugins/withLockedRanges";

function makeTranscriptBlock(blockId: string, text: string): TranscriptBlockElement {
  return {
    type: "transcript-block",
    blockId,
    sequence: 1,
    upstreamText: text,
    startTimestampMs: 100,
    language: "ja",
    children: [{ text }],
  };
}

function createLockedEditor() {
  const editor = withLockedRanges(createEditor());
  editor.children = [makeTranscriptBlock("block-1", "hello world")];
  return editor;
}

describe("LockManager.lockSelection", () => {
  test("applies locked mark to the selected range", () => {
    const editor = createLockedEditor();
    Transforms.select(editor, {
      anchor: { path: [0, 0], offset: 0 },
      focus: { path: [0, 0], offset: 5 },
    });

    const lockRange = lockSelection(editor);

    expect(lockRange).not.toBeNull();
    expect(lockRange?.blockId).toBe("block-1");
    const block = editor.children[0] as TranscriptBlockElement;
    const lockedLeaf = block.children.find((child) => child.locked === true);
    expect(lockedLeaf?.text).toBe("hello");
  });
});

describe("LockManager.lockAtPath", () => {
  test("applies locked mark to the text leaf at path", () => {
    const editor = createLockedEditor();
    Transforms.select(editor, {
      anchor: { path: [0, 0], offset: 2 },
      focus: { path: [0, 0], offset: 2 },
    });

    const lockRange = lockAtPath(editor, [0, 0]);

    expect(lockRange).not.toBeNull();
    expect(lockRange?.blockId).toBe("block-1");
    const block = editor.children[0] as TranscriptBlockElement;
    expect(block.children[0]?.locked).toBe(true);
  });
});
