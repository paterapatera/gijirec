import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { act, cleanup, render } from "@testing-library/react";
import { createRef } from "react";
import { AI_TRANSCRIPT_SCROLL_STYLE } from "../../application/transcript/plugins/withStableSelection";
import type { TranscriptBlockView } from "../../domain/transcript/types";
import { setupTestDom } from "../../test-setup";
import { AiTranscriptEditor, type AiTranscriptEditorRef } from "./AiTranscriptEditor";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

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
  act(() => {
    // Flush Slate/React updates triggered by editor.onChange().
  });
}

describe("AiTranscriptEditor", () => {
  test("renders with data-testid and jagged-ice background", () => {
    const { getByTestId } = render(<AiTranscriptEditor />);
    const scrollContainer = getByTestId("ai-transcript-editor");
    const panel = scrollContainer.closest(".ai-transcript-editor-panel");

    expect(scrollContainer).toBeTruthy();
    expect(panel).toBeTruthy();
    expect((panel as HTMLElement).style.backgroundColor).toBe("var(--jagged-ice)");
  });

  test("scroll container uses AI_TRANSCRIPT_SCROLL_STYLE", () => {
    const { getByTestId } = render(<AiTranscriptEditor />);
    const scrollContainer = getByTestId("ai-transcript-editor");

    expect(scrollContainer.style.overflowY).toBe(AI_TRANSCRIPT_SCROLL_STYLE.overflowY);
    expect(scrollContainer.style.overflowAnchor).toBe(AI_TRANSCRIPT_SCROLL_STYLE.overflowAnchor);
  });

  test("getBlocks returns empty array initially", () => {
    const ref = createRef<AiTranscriptEditorRef>();
    render(<AiTranscriptEditor ref={ref} />);

    expect(ref.current?.getBlocks()).toEqual([]);
  });

  test("appendBlock inserts at end with blockId and startTimestampMs on transcript-block", () => {
    const ref = createRef<AiTranscriptEditorRef>();
    const { container } = render(<AiTranscriptEditor ref={ref} />);

    appendBlock(ref.current, makeBlock("block-1", "こんにちは", 1, 1500));

    const blockEl = container.querySelector("[data-transcript-block]");
    expect(blockEl).toBeTruthy();
    expect(blockEl?.getAttribute("data-block-id")).toBe("block-1");
    expect(blockEl?.getAttribute("data-start-timestamp-ms")).toBe("1500");
    expect(blockEl?.textContent).toBe("こんにちは");
  });

  test("appendBlock updates display without remounting the scroll container", () => {
    const ref = createRef<AiTranscriptEditorRef>();
    const { getByTestId } = render(<AiTranscriptEditor ref={ref} />);
    const scrollBefore = getByTestId("ai-transcript-editor");

    appendBlock(ref.current, makeBlock("block-1", "first", 1));
    appendBlock(ref.current, makeBlock("block-2", "second", 2));

    const scrollAfter = getByTestId("ai-transcript-editor");
    expect(scrollAfter).toBe(scrollBefore);
    expect(ref.current?.getBlocks()).toHaveLength(2);
    expect(ref.current?.getBlocks()[1]?.displayText).toBe("second");
  });

  test("getBlocks returns TranscriptBlockView snapshots for SaveOrchestrator", () => {
    const ref = createRef<AiTranscriptEditorRef>();
    render(<AiTranscriptEditor ref={ref} />);

    appendBlock(ref.current, makeBlock("block-1", "alpha", 1, 200));
    appendBlock(ref.current, makeBlock("block-2", "beta", 2, 300));

    expect(ref.current?.getBlocks()).toEqual([
      {
        blockId: "block-1",
        sequence: 1,
        text: "alpha",
        displayText: "alpha",
        startTimestampMs: 200,
        language: "ja",
      },
      {
        blockId: "block-2",
        sequence: 2,
        text: "beta",
        displayText: "beta",
        startTimestampMs: 300,
        language: "ja",
      },
    ]);
  });

  test("does not import editorCommands (no automatic disk write)", () => {
    const source = readFileSync(join(import.meta.dir, "AiTranscriptEditor.tsx"), "utf8");
    expect(source).not.toContain("editorCommands");
  });

  test("syncs blocks from props into the document", () => {
    const { container, rerender } = render(<AiTranscriptEditor blocks={[]} />);

    act(() => {
      rerender(<AiTranscriptEditor blocks={[makeBlock("block-1", "こんにちは", 1, 1500)]} />);
    });

    const blockEl = container.querySelector("[data-transcript-block]");
    expect(blockEl).toBeTruthy();
    expect(blockEl?.getAttribute("data-block-id")).toBe("block-1");
    expect(blockEl?.textContent).toBe("こんにちは");
  });
});
