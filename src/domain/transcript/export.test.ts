import { describe, expect, test } from "bun:test";
import { type AiTranscriptionJsonlRecord, toAiMarkdown, toJsonlRecords } from "./export";
import type { TranscriptBlockView } from "./types";

function makeBlock(
  overrides: Partial<TranscriptBlockView> & Pick<TranscriptBlockView, "blockId">,
): TranscriptBlockView {
  return {
    sequence: 1,
    text: "original",
    startTimestampMs: 12_345,
    language: "ja",
    displayText: "original",
    ...overrides,
  };
}

describe("toAiMarkdown", () => {
  test("returns empty string for empty document", () => {
    expect(toAiMarkdown([])).toBe("");
  });

  test("concatenates displayText without timestamps", () => {
    const blocks: TranscriptBlockView[] = [
      makeBlock({
        blockId: "a",
        sequence: 1,
        displayText: "こんにちは",
        startTimestampMs: 1000,
      }),
      makeBlock({
        blockId: "b",
        sequence: 2,
        displayText: "世界",
        startTimestampMs: 2000,
      }),
    ];

    const markdown = toAiMarkdown(blocks);

    expect(markdown).toBe("こんにちは\n世界");
    expect(markdown).not.toContain("1000");
    expect(markdown).not.toContain("2000");
    expect(markdown).not.toMatch(/\d{4,}/);
  });

  test("uses locked manual-corrected displayText instead of original text", () => {
    const blocks: TranscriptBlockView[] = [
      makeBlock({
        blockId: "a",
        sequence: 1,
        text: "こんにちわ",
        displayText: "こんにちは",
        startTimestampMs: 500,
      }),
    ];

    expect(toAiMarkdown(blocks)).toBe("こんにちは");
    expect(toAiMarkdown(blocks)).not.toContain("こんにちわ");
  });
});

describe("toJsonlRecords", () => {
  test("returns empty array for empty document", () => {
    expect(toJsonlRecords([])).toEqual([]);
  });

  test("produces contract-shaped records with start_timestamp_ms", () => {
    const blocks: TranscriptBlockView[] = [
      makeBlock({
        blockId: "550e8400-e29b-41d4-a716-446655440000",
        sequence: 1,
        displayText: "hello",
        startTimestampMs: 12_345,
        language: "en",
      }),
    ];

    const records = toJsonlRecords(blocks);

    expect(records).toEqual([
      {
        block_id: "550e8400-e29b-41d4-a716-446655440000",
        sequence: 1,
        text: "hello",
        start_timestamp_ms: 12_345,
        language: "en",
      } satisfies AiTranscriptionJsonlRecord,
    ]);
  });

  test("uses displayText for text field after manual lock/edit", () => {
    const blocks: TranscriptBlockView[] = [
      makeBlock({
        blockId: "block-1",
        sequence: 2,
        text: "wrong",
        displayText: "corrected",
        startTimestampMs: 999,
        language: "ja",
      }),
    ];

    const records = toJsonlRecords(blocks);

    expect(records[0]?.text).toBe("corrected");
    expect(records[0]?.text).not.toBe("wrong");
    expect(records[0]?.start_timestamp_ms).toBe(999);
  });

  test("preserves sequence order in output records", () => {
    const blocks: TranscriptBlockView[] = [
      makeBlock({ blockId: "a", sequence: 1, displayText: "first" }),
      makeBlock({ blockId: "b", sequence: 2, displayText: "second" }),
    ];

    const records = toJsonlRecords(blocks);

    expect(records.map((r) => r.sequence)).toEqual([1, 2]);
    expect(records.map((r) => r.text)).toEqual(["first", "second"]);
  });
});
