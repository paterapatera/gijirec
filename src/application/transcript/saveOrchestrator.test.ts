import { describe, expect, test } from "bun:test";
import { toJsonlRecords } from "../../domain/transcript/export";
import type { EditorSettings, TranscriptBlockView } from "../../domain/transcript/types";
import {
  createSaveOrchestrator,
  type SaveTranscriptSessionRequest,
  type SaveTranscriptSessionResult,
} from "./saveOrchestrator";

function makeBlock(blockId: string, displayText: string, sequence = 1): TranscriptBlockView {
  return {
    blockId,
    sequence,
    text: displayText,
    displayText,
    startTimestampMs: 100,
    language: "ja",
  };
}

const defaultSettings: EditorSettings = {
  save_directory: "/tmp/saves",
  export_jsonl_enabled: false,
};

function createSessionInput(
  overrides: {
    handwriting?: string;
    blocks?: TranscriptBlockView[];
    settings?: EditorSettings;
    sessionId?: string;
  } = {},
) {
  return {
    handwritingEditor: { getPlainText: () => overrides.handwriting ?? "" },
    aiEditor: { getBlocks: () => overrides.blocks ?? [] },
    settings: overrides.settings ?? defaultSettings,
    sessionId: overrides.sessionId ?? "session-1",
  };
}

describe("SaveOrchestrator.saveSession", () => {
  test("serializes snapshot via export helpers and invokes saveFn", async () => {
    const blocks = [makeBlock("a", "こんにちは", 1), makeBlock("b", "世界", 2)];
    let captured: SaveTranscriptSessionRequest | undefined;
    const saveFn = async (request: SaveTranscriptSessionRequest) => {
      captured = request;
      return {
        success: true,
        output_directory: "/tmp/saves/2026/09/06/10_00_00",
        files_written: ["/tmp/saves/2026/09/06/10_00_00/handwriting.md"],
      } satisfies SaveTranscriptSessionResult;
    };

    const orchestrator = createSaveOrchestrator({ saveFn });
    const result = await orchestrator.saveSession(
      createSessionInput({ handwriting: "# notes", blocks }),
    );

    expect(captured).toEqual({
      session_id: "session-1",
      handwriting_markdown: "# notes",
      ai_transcription_markdown: "こんにちは\n世界",
    });
    expect(result.success).toBe(true);
  });

  test("excludes blocks that arrive after save snapshot", async () => {
    let blocks = [makeBlock("a", "first", 1)];
    const aiEditor = {
      getBlocks: () => [...blocks],
    };

    let captured: SaveTranscriptSessionRequest | undefined;
    const saveFn = async (request: SaveTranscriptSessionRequest) => {
      captured = request;
      blocks = [...blocks, makeBlock("b", "second", 2)];
      return {
        success: true,
        files_written: ["/out/ai-transcription.md"],
      } satisfies SaveTranscriptSessionResult;
    };

    const orchestrator = createSaveOrchestrator({ saveFn });
    await orchestrator.saveSession({
      handwritingEditor: { getPlainText: () => "" },
      aiEditor,
      settings: defaultSettings,
      sessionId: "session-2",
    });

    expect(captured?.ai_transcription_markdown).toBe("first");
    expect(captured?.ai_transcription_markdown).not.toContain("second");
  });

  test("omits ai_transcription_jsonl when export_jsonl_enabled is false", async () => {
    let captured: SaveTranscriptSessionRequest | undefined;
    const saveFn = async (request: SaveTranscriptSessionRequest) => {
      captured = request;
      return { success: true, files_written: ["/out/handwriting.md"] };
    };

    const orchestrator = createSaveOrchestrator({ saveFn });
    await orchestrator.saveSession(
      createSessionInput({
        blocks: [makeBlock("a", "hello")],
        settings: { save_directory: "/tmp", export_jsonl_enabled: false },
      }),
    );

    expect(captured?.ai_transcription_jsonl).toBeUndefined();
  });

  test("includes ai_transcription_jsonl when export_jsonl_enabled is true", async () => {
    const blocks = [makeBlock("a", "hello")];
    let captured: SaveTranscriptSessionRequest | undefined;
    const saveFn = async (request: SaveTranscriptSessionRequest) => {
      captured = request;
      return { success: true, files_written: ["/out/ai-transcription.jsonl"] };
    };

    const orchestrator = createSaveOrchestrator({ saveFn });
    await orchestrator.saveSession(
      createSessionInput({
        blocks,
        settings: { save_directory: "/tmp", export_jsonl_enabled: true },
      }),
    );

    expect(captured?.ai_transcription_jsonl).toEqual(toJsonlRecords(blocks));
  });

  test("sends empty markdown for empty handwriting and ai transcript", async () => {
    let captured: SaveTranscriptSessionRequest | undefined;
    const saveFn = async (request: SaveTranscriptSessionRequest) => {
      captured = request;
      return { success: true, files_written: ["/out/handwriting.md", "/out/ai-transcription.md"] };
    };

    const orchestrator = createSaveOrchestrator({ saveFn });
    await orchestrator.saveSession(createSessionInput());

    expect(captured?.handwriting_markdown).toBe("");
    expect(captured?.ai_transcription_markdown).toBe("");
  });

  test("still invokes saveFn when save_directory is null", async () => {
    let called = false;
    const saveFn = async () => {
      called = true;
      return {
        success: false,
        error: {
          code: "SAVE_DIRECTORY_NOT_SET",
          message_ja: "保存先が未設定です",
          action_ja: "保存先を選択してください",
          recoverable: true,
        },
      } satisfies SaveTranscriptSessionResult;
    };

    const orchestrator = createSaveOrchestrator({ saveFn });
    await orchestrator.saveSession(
      createSessionInput({
        settings: { save_directory: null, export_jsonl_enabled: false },
      }),
    );

    expect(called).toBe(true);
  });
});

describe("SaveOrchestrator.isSaving guard", () => {
  test("isSaving is true while save is in flight", async () => {
    let resolveSave!: (result: SaveTranscriptSessionResult) => void;
    const saveFn = () =>
      new Promise<SaveTranscriptSessionResult>((resolve) => {
        resolveSave = resolve;
      });

    const orchestrator = createSaveOrchestrator({ saveFn });
    expect(orchestrator.isSaving).toBe(false);

    const pending = orchestrator.saveSession(createSessionInput());
    expect(orchestrator.isSaving).toBe(true);

    resolveSave({ success: true, files_written: ["/out/handwriting.md"] });
    await pending;

    expect(orchestrator.isSaving).toBe(false);
  });

  test("second save while in flight returns in-flight result without re-invoking saveFn", async () => {
    let resolveSave!: (result: SaveTranscriptSessionResult) => void;
    let callCount = 0;
    const saveFn = async () => {
      callCount += 1;
      return new Promise<SaveTranscriptSessionResult>((resolve) => {
        resolveSave = resolve;
      });
    };

    const orchestrator = createSaveOrchestrator({ saveFn });
    const input = createSessionInput({ sessionId: "session-guard" });

    const first = orchestrator.saveSession(input);
    const second = orchestrator.saveSession(input);

    expect(callCount).toBe(1);
    expect(orchestrator.isSaving).toBe(true);

    const expected: SaveTranscriptSessionResult = {
      success: true,
      files_written: ["/out/handwriting.md"],
    };
    resolveSave(expected);

    const [firstResult, secondResult] = await Promise.all([first, second]);
    expect(firstResult).toEqual(expected);
    expect(secondResult).toEqual(expected);
    expect(callCount).toBe(1);
    expect(orchestrator.isSaving).toBe(false);
  });
});
