import { describe, expect, test } from "bun:test";
import type { ReactElement } from "react";
import { createEditor, Transforms } from "slate";
import type { TranscriptBlockElement } from "../../../domain/transcript/slateTypes";
import { lockSelection } from "../lockManager";
import { renderLockedLeaf, withLockedRanges } from "./withLockedRanges";

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

function createLockedRangesEditor() {
  const editor = withLockedRanges(createEditor());
  editor.children = [makeTranscriptBlock("block-1", "hello world")];
  return editor;
}

function selectAndLock(
  editor: ReturnType<typeof createLockedRangesEditor>,
  start: number,
  end: number,
) {
  Transforms.select(editor, {
    anchor: { path: [0, 0], offset: start },
    focus: { path: [0, 0], offset: end },
  });
  lockSelection(editor);
}

describe("withLockedRanges auto-lock", () => {
  test("locks range automatically when selection is committed", () => {
    const editor = createLockedRangesEditor();
    Transforms.select(editor, {
      anchor: { path: [0, 0], offset: 0 },
      focus: { path: [0, 0], offset: 5 },
    });

    Transforms.select(editor, {
      anchor: { path: [0, 0], offset: 6 },
      focus: { path: [0, 0], offset: 6 },
    });

    const block = editor.children[0] as TranscriptBlockElement;
    const lockedLeaf = block.children.find((child) => child.locked === true);
    expect(lockedLeaf?.text).toBe("hello");
  });

  test("locks text automatically on direct user typing", () => {
    const editor = createLockedRangesEditor();
    Transforms.select(editor, {
      anchor: { path: [0, 0], offset: 5 },
      focus: { path: [0, 0], offset: 5 },
    });

    Transforms.insertText(editor, "!");

    const block = editor.children[0] as TranscriptBlockElement;
    const editedLeaf = block.children.find((child) => child.text.includes("!"));
    expect(editedLeaf?.locked).toBe(true);
  });
});

describe("withLockedRanges upstream protection", () => {
  test("rejects upstream remove_text on locked text", () => {
    const editor = createLockedRangesEditor();
    selectAndLock(editor, 0, 5);

    editor.applyUpstream({
      type: "remove_text",
      path: [0, 0],
      offset: 0,
      text: "hello",
    });

    const block = editor.children[0] as TranscriptBlockElement;
    expect(block.children.some((child) => child.locked === true && child.text === "hello")).toBe(
      true,
    );
  });

  test("rejects upstream insert_text on locked leaf", () => {
    const editor = createLockedRangesEditor();
    selectAndLock(editor, 0, 5);

    editor.applyUpstream({
      type: "insert_text",
      path: [0, 0],
      offset: 5,
      text: "X",
    });

    const block = editor.children[0] as TranscriptBlockElement;
    const lockedLeaf = block.children.find((child) => child.locked === true);
    expect(lockedLeaf?.text).toBe("hello");
  });

  test("rejects upstream set_node that replaces locked text", () => {
    const editor = createLockedRangesEditor();
    selectAndLock(editor, 0, 5);

    editor.applyUpstream({
      type: "set_node",
      path: [0, 0],
      properties: { text: "hello", locked: true },
      newProperties: { text: "replaced", locked: true },
    });

    const block = editor.children[0] as TranscriptBlockElement;
    const lockedLeaf = block.children.find((child) => child.locked === true);
    expect(lockedLeaf?.text).toBe("hello");
  });

  test("requires applyUpstream for upstream ops — direct apply does not protect locked text", () => {
    const editor = createLockedRangesEditor();
    selectAndLock(editor, 0, 5);

    editor.apply({
      type: "remove_text",
      path: [0, 0],
      offset: 0,
      text: "hello",
    });

    const block = editor.children[0] as TranscriptBlockElement;
    expect(block.children.some((child) => child.locked === true && child.text === "hello")).toBe(
      false,
    );
  });

  test("allows user edits on locked text", () => {
    const editor = createLockedRangesEditor();
    selectAndLock(editor, 0, 5);

    Transforms.insertText(editor, "!", { at: { path: [0, 0], offset: 5 } });

    const block = editor.children[0] as TranscriptBlockElement;
    const lockedLeaf = block.children.find((child) => child.locked === true);
    expect(lockedLeaf?.text).toBe("hello!");
  });

  test("keeps locked text unchanged when upstream appends a new block", () => {
    const editor = createLockedRangesEditor();
    selectAndLock(editor, 0, 5);

    Transforms.insertNodes(editor, makeTranscriptBlock("block-2", " appended"), {
      at: [editor.children.length],
    });

    const firstBlock = editor.children[0] as TranscriptBlockElement;
    const lockedLeaf = firstBlock.children.find((child) => child.locked === true);
    expect(lockedLeaf?.text).toBe("hello");
    expect(editor.children).toHaveLength(2);
  });

  test("rejects upstream modification of locked text while allowing unlocked tail updates", () => {
    const editor = createLockedRangesEditor();
    selectAndLock(editor, 0, 5);

    editor.applyUpstream({
      type: "remove_text",
      path: [0, 1],
      offset: 1,
      text: "world",
    });
    editor.applyUpstream({
      type: "insert_text",
      path: [0, 1],
      offset: 1,
      text: "there",
    });

    const block = editor.children[0] as TranscriptBlockElement;
    const lockedLeaf = block.children.find((child) => child.locked === true);
    expect(lockedLeaf?.text).toBe("hello");
    const unlockedLeaf = block.children.find((child) => !child.locked);
    expect(unlockedLeaf?.text).toBe(" there");

    editor.applyUpstream({
      type: "remove_text",
      path: [0, 0],
      offset: 0,
      text: "hello",
    });
    const afterUpstream = editor.children[0] as TranscriptBlockElement;
    const stillLocked = afterUpstream.children.find((child) => child.locked === true);
    expect(stillLocked?.text).toBe("hello");
  });

  test("allows user to re-edit locked range while keeping the lock mark", () => {
    const editor = createLockedRangesEditor();
    selectAndLock(editor, 0, 5);

    Transforms.select(editor, {
      anchor: { path: [0, 0], offset: 0 },
      focus: { path: [0, 0], offset: 5 },
    });
    Transforms.insertText(editor, "HELLO");

    const block = editor.children[0] as TranscriptBlockElement;
    const lockedLeaf = block.children.find((child) => child.locked === true);
    expect(lockedLeaf?.text).toBe("HELLO");
    expect(lockedLeaf?.locked).toBe(true);
  });
});

describe("renderLockedLeaf", () => {
  test("applies classic-rose background and plum underline for locked leaves", () => {
    const element = renderLockedLeaf({
      attributes: { "data-slate-leaf": true },
      children: "locked",
      leaf: { text: "locked", locked: true },
      text: { text: "locked", locked: true },
    }) as ReactElement<{ style?: Record<string, string> }>;

    expect(element.props.style).toEqual({
      backgroundColor: "var(--classic-rose)",
      textDecoration: "underline",
      textDecorationColor: "var(--plum)",
    });
  });

  test("renders unlocked leaves without lock styling", () => {
    const element = renderLockedLeaf({
      attributes: { "data-slate-leaf": true },
      children: "plain",
      leaf: { text: "plain" },
      text: { text: "plain" },
    }) as ReactElement<{ style?: Record<string, string> }>;

    expect(element.props.style).toBeUndefined();
  });
});
