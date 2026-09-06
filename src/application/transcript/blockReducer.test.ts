import { describe, expect, test } from "bun:test";
import type { TranscriptBlockView } from "../../domain/transcript/types";
import {
  appendBlock,
  createTranscriptSessionState,
  type TranscriptSessionState,
} from "./blockReducer";

function makeBlock(
  overrides: Partial<TranscriptBlockView> & Pick<TranscriptBlockView, "blockId">,
): TranscriptBlockView {
  return {
    sequence: 1,
    text: "text",
    startTimestampMs: 100,
    language: "ja",
    displayText: "text",
    ...overrides,
  };
}

describe("BlockReducer", () => {
  test("appends the first block to empty state", () => {
    const state = createTranscriptSessionState();
    const block = makeBlock({ blockId: "a", sequence: 1, displayText: "first" });

    const next = appendBlock(state, block);

    expect(next.blocks).toHaveLength(1);
    expect(next.blocks[0]).toEqual(block);
    expect(next.sequenceGapCount).toBe(0);
  });

  test("appends subsequent blocks in order without gaps", () => {
    let state = createTranscriptSessionState();
    const first = makeBlock({ blockId: "a", sequence: 1, displayText: "first" });
    const second = makeBlock({ blockId: "b", sequence: 2, displayText: "second" });

    state = appendBlock(state, first);
    state = appendBlock(state, second);

    expect(state.blocks.map((b) => b.displayText)).toEqual(["first", "second"]);
    expect(state.sequenceGapCount).toBe(0);
  });

  test("does not mutate existing blocks when the same blockId is appended again", () => {
    let state = createTranscriptSessionState();
    const original = makeBlock({
      blockId: "a",
      sequence: 1,
      text: "original",
      displayText: "original",
    });

    state = appendBlock(state, original);
    const blocksAfterFirst = state.blocks;

    state = appendBlock(
      state,
      makeBlock({
        blockId: "a",
        sequence: 1,
        text: "mutated",
        displayText: "mutated",
      }),
    );

    expect(state.blocks).toHaveLength(1);
    expect(state.blocks[0]).toBe(blocksAfterFirst[0]);
    expect(state.blocks[0]?.displayText).toBe("original");
    expect(state.blocks[0]?.text).toBe("original");
  });

  test("increments sequenceGapCount when sequence skips expected value", () => {
    let state = createTranscriptSessionState();

    state = appendBlock(state, makeBlock({ blockId: "a", sequence: 1 }));
    state = appendBlock(state, makeBlock({ blockId: "b", sequence: 3 }));

    expect(state.blocks).toHaveLength(2);
    expect(state.sequenceGapCount).toBe(1);
  });

  test("accumulates multiple missing sequences in one gap", () => {
    let state = createTranscriptSessionState();

    state = appendBlock(state, makeBlock({ blockId: "a", sequence: 1 }));
    state = appendBlock(state, makeBlock({ blockId: "b", sequence: 5 }));

    expect(state.sequenceGapCount).toBe(3);
  });

  test("keeps in-memory state without an auto-clear reset", () => {
    let state: TranscriptSessionState = createTranscriptSessionState();
    state = appendBlock(state, makeBlock({ blockId: "a", sequence: 1 }));

    expect(state.blocks).toHaveLength(1);
    expect(typeof (state as { reset?: unknown }).reset).toBe("undefined");
  });
});
