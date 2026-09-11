import { describe, expect, test } from "bun:test";
import type { CustomText, TranscriptBlockElement } from "./slateTypes";
import {
  DEFAULT_EDITOR_SETTINGS,
  type EditorSettings,
  type LockRange,
  mapBlockFromContract,
  type TranscriptBlockContract,
  type TranscriptBlockView,
} from "./types";

describe("TranscriptBlockView", () => {
  test("mapBlockFromContract maps snake_case contract fields to camelCase view", () => {
    const contract: TranscriptBlockContract = {
      block_id: "550e8400-e29b-41d4-a716-446655440000",
      sequence: 1,
      text: "こんにちは",
      start_timestamp_ms: 12_345,
      language: "ja",
    };

    const view = mapBlockFromContract(contract);

    expect(view).toEqual({
      blockId: contract.block_id,
      sequence: contract.sequence,
      text: contract.text,
      startTimestampMs: contract.start_timestamp_ms,
      language: contract.language,
      displayText: contract.text,
    } satisfies TranscriptBlockView);
  });

  test("TranscriptBlockView exposes required camelCase fields", () => {
    const view: TranscriptBlockView = {
      blockId: "id",
      sequence: 2,
      text: "hello",
      startTimestampMs: 100,
      language: "en",
      displayText: "hello",
    };

    expect(view.blockId).toBe("id");
    expect(view.sequence).toBe(2);
    expect(view.text).toBe("hello");
    expect(view.startTimestampMs).toBe(100);
    expect(view.language).toBe("en");
    expect(view.displayText).toBe("hello");
  });
});

describe("EditorSettings", () => {
  test("mirrors contract shape with snake_case fields", () => {
    const settings: EditorSettings = {
      save_directory: "/tmp/gijirec",
      export_jsonl_enabled: true,
    };

    expect(settings.save_directory).toBe("/tmp/gijirec");
    expect(settings.export_jsonl_enabled).toBe(true);
  });

  test("default settings match contract defaults", () => {
    expect(DEFAULT_EDITOR_SETTINGS).toEqual({
      save_directory: null,
      export_jsonl_enabled: false,
    });
  });
});

describe("LockRange", () => {
  test("combines Slate range with blockId", () => {
    const lock: LockRange = {
      blockId: "550e8400-e29b-41d4-a716-446655440000",
      range: {
        anchor: { path: [0, 0], offset: 0 },
        focus: { path: [0, 0], offset: 3 },
      },
    };

    expect(lock.blockId).toBe("550e8400-e29b-41d4-a716-446655440000");
    expect(lock.range.anchor.offset).toBe(0);
    expect(lock.range.focus.offset).toBe(3);
  });
});

describe("Slate transcript types", () => {
  test("TranscriptBlockElement and CustomText match design shape", () => {
    const lockedText: CustomText = { text: "locked", locked: true };
    const plainText: CustomText = { text: "plain" };

    const element: TranscriptBlockElement = {
      type: "transcript-block",
      blockId: "block-1",
      sequence: 1,
      upstreamText: "plainlocked",
      startTimestampMs: 500,
      language: "ja",
      children: [plainText, lockedText],
    };

    expect(element.type).toBe("transcript-block");
    expect(element.children[1]?.locked).toBe(true);
  });
});
