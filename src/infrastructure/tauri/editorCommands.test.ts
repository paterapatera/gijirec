import { describe, expect, test } from "bun:test";
import type { AiTranscriptionJsonlRecord } from "../../domain/transcript/export";
import type { EditorSettings } from "../../domain/transcript/types";
import {
  getEditorSettings,
  pickSaveDirectory,
  type SaveTranscriptSessionRequest,
  type SaveTranscriptSessionResult,
  saveTranscriptSession,
  setEditorSettings,
} from "./editorCommands";

type InvokeCall = {
  command: string;
  args?: unknown;
};

function createMockInvoke<T>(response: T) {
  const calls: InvokeCall[] = [];
  const invokeFn = async (command: string, args?: unknown) => {
    calls.push({ command, args });
    return response;
  };
  return { invokeFn, calls };
}

describe("editorCommands", () => {
  test("saveTranscriptSession invokes save_transcript_session with request payload", async () => {
    const request: SaveTranscriptSessionRequest = {
      session_id: "session-1",
      handwriting_markdown: "# notes",
      ai_transcription_markdown: "hello world",
      ai_transcription_jsonl: [
        {
          block_id: "block-1",
          sequence: 1,
          text: "hello world",
          start_timestamp_ms: 100,
          language: "ja",
        } satisfies AiTranscriptionJsonlRecord,
      ],
    };
    const result: SaveTranscriptSessionResult = {
      success: true,
      output_directory: "/data/2026/09/06/10_00_00",
      files_written: ["/data/2026/09/06/10_00_00/handwriting.md"],
    };
    const { invokeFn, calls } = createMockInvoke(result);

    const actual = await saveTranscriptSession(request, { invokeFn });

    expect(calls).toEqual([{ command: "save_transcript_session", args: request }]);
    expect(actual).toEqual(result);
  });

  test("getEditorSettings invokes get_editor_settings without args", async () => {
    const settings: EditorSettings = {
      save_directory: "/tmp/saves",
      export_jsonl_enabled: true,
    };
    const { invokeFn, calls } = createMockInvoke(settings);

    const actual = await getEditorSettings({ invokeFn });

    expect(calls).toEqual([{ command: "get_editor_settings" }]);
    expect(actual).toEqual(settings);
  });

  test("setEditorSettings invokes set_editor_settings with partial update", async () => {
    const request = { export_jsonl_enabled: false };
    const settings: EditorSettings = {
      save_directory: "/tmp/saves",
      export_jsonl_enabled: false,
    };
    const { invokeFn, calls } = createMockInvoke(settings);

    const actual = await setEditorSettings(request, { invokeFn });

    expect(calls).toEqual([{ command: "set_editor_settings", args: request }]);
    expect(actual).toEqual(settings);
  });

  test("pickSaveDirectory invokes pick_save_directory without args", async () => {
    const { invokeFn, calls } = createMockInvoke("/tmp/picked");

    const actual = await pickSaveDirectory({ invokeFn });

    expect(calls).toEqual([{ command: "pick_save_directory" }]);
    expect(actual).toBe("/tmp/picked");
  });

  test("pickSaveDirectory returns null when dialog is cancelled", async () => {
    const { invokeFn } = createMockInvoke(null);

    const actual = await pickSaveDirectory({ invokeFn });

    expect(actual).toBeNull();
  });
});
