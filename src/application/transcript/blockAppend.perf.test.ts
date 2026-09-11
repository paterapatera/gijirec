/**
 * Light microbench for design Performance/Load #1:
 * 500 sequential block appends — p95 single-append latency < 16 ms.
 */
import { beforeAll, describe, expect, test } from "bun:test";
import { Editor } from "slate";
import type { TranscriptBlockElement } from "../../domain/transcript/slateTypes";
import { setupTestDom } from "../../test-setup";
import { createAiTranscriptEditor } from "./createAiTranscriptEditor";

const BLOCK_COUNT = 500;
const P95_THRESHOLD_MS = 16;

beforeAll(() => {
  setupTestDom();
});

function makeTranscriptBlock(blockId: string, sequence: number): TranscriptBlockElement {
  return {
    type: "transcript-block",
    blockId,
    sequence,
    upstreamText: `block-${sequence}`,
    startTimestampMs: 100 + sequence,
    language: "ja",
    children: [{ text: `block-${sequence}` }],
  };
}

function percentile(sorted: number[], p: number): number {
  if (sorted.length === 0) {
    return 0;
  }
  const index = Math.ceil((p / 100) * sorted.length) - 1;
  const clampedIndex = Math.max(0, Math.min(index, sorted.length - 1));
  return sorted[clampedIndex] ?? 0;
}

describe("block append performance", () => {
  test(`p95 append latency < ${P95_THRESHOLD_MS} ms over ${BLOCK_COUNT} blocks`, () => {
    const editor = createAiTranscriptEditor();
    const samplesMs: number[] = [];

    // Warmup: one append outside measured window.
    Editor.withoutNormalizing(editor, () => {
      editor.applyUpstream({
        type: "insert_node",
        path: [0],
        node: makeTranscriptBlock("warmup", 0),
      });
    });
    editor.children = [];

    for (let i = 0; i < BLOCK_COUNT; i += 1) {
      const block = makeTranscriptBlock(`block-${i}`, i + 1);
      const start = performance.now();
      Editor.withoutNormalizing(editor, () => {
        editor.applyUpstream({
          type: "insert_node",
          path: [editor.children.length],
          node: block,
        });
      });
      samplesMs.push(performance.now() - start);
    }

    expect(editor.children).toHaveLength(BLOCK_COUNT);

    const sorted = [...samplesMs].sort((a, b) => a - b);
    const p50 = percentile(sorted, 50);
    const p95 = percentile(sorted, 95);
    const max = sorted[sorted.length - 1] ?? 0;

    // Expose measurements for validation-checklist capture (bun test stdout).
    console.log(
      JSON.stringify({
        bench: "block_append_500",
        block_count: BLOCK_COUNT,
        p50_ms: Number(p50.toFixed(3)),
        p95_ms: Number(p95.toFixed(3)),
        max_ms: Number(max.toFixed(3)),
        threshold_p95_ms: P95_THRESHOLD_MS,
      }),
    );

    expect(p95).toBeLessThan(P95_THRESHOLD_MS);
  });
});
