import { beforeAll, beforeEach, describe, expect, mock, test } from "bun:test";
import type { SaveTranscriptSessionResult } from "../../infrastructure/tauri/editorCommands";

const mockToastSuccess = mock(() => {});
const mockToastError = mock(() => {});

mock.module("sonner", () => ({
  toast: {
    success: mockToastSuccess,
    error: mockToastError,
  },
}));

let showSaveResult: (result: SaveTranscriptSessionResult) => void;

beforeAll(async () => {
  const mod = await import("./SaveResultToast");
  showSaveResult = mod.showSaveResult;
});

beforeEach(() => {
  mockToastSuccess.mockClear();
  mockToastError.mockClear();
});

function stringifyToastArgs(args: unknown[]): string {
  return JSON.stringify(args);
}

describe("showSaveResult", () => {
  test("shows output_directory via toast.success when save succeeds", () => {
    const outputDirectory = "C:\\Users\\test\\Documents\\gijirec\\2026-09-06_1430";
    const result: SaveTranscriptSessionResult = {
      success: true,
      output_directory: outputDirectory,
      files_written: ["handwriting.md", "ai_transcription.md"],
    };

    showSaveResult(result);

    expect(mockToastSuccess).toHaveBeenCalledTimes(1);
    expect(mockToastError).not.toHaveBeenCalled();

    const [, options] = mockToastSuccess.mock.calls[0]! as [string, { description?: string }];
    expect(options.description).toBe(outputDirectory);
  });

  test("shows message_ja and action_ja via toast.error when save fails", () => {
    const result: SaveTranscriptSessionResult = {
      success: false,
      error: {
        code: "SAVE_DIRECTORY_NOT_SET",
        message_ja: "保存先フォルダが設定されていません",
        action_ja: "設定画面で保存先フォルダを選択してください",
        recoverable: true,
      },
    };

    showSaveResult(result);

    expect(mockToastError).toHaveBeenCalledTimes(1);
    expect(mockToastSuccess).not.toHaveBeenCalled();

    const args = mockToastError.mock.calls[0]!;
    const payload = stringifyToastArgs(args);
    expect(args[0]).toBe(result.error!.message_ja);
    expect(payload).toContain(result.error!.action_ja);
  });

  test("does not display error code as user-facing toast text", () => {
    const result: SaveTranscriptSessionResult = {
      success: false,
      error: {
        code: "SAVE_FILE_WRITE_FAILED",
        message_ja: "ファイルの保存に失敗しました",
        action_ja: "ディスク容量と権限を確認してから再試行してください",
        recoverable: true,
      },
    };

    showSaveResult(result);

    const payload = stringifyToastArgs(mockToastError.mock.calls[0]!);
    expect(payload).not.toContain("SAVE_FILE_WRITE_FAILED");
    expect(payload).toContain(result.error!.message_ja);
    expect(payload).toContain(result.error!.action_ja);
  });

  test("does not include transcript file contents in toast payload", () => {
    const result: SaveTranscriptSessionResult = {
      success: true,
      output_directory: "/tmp/gijirec/output",
      files_written: ["handwriting.md", "ai_transcription.md"],
    };

    showSaveResult(result);

    const payload = stringifyToastArgs(mockToastSuccess.mock.calls[0]!);
    expect(payload).toContain("/tmp/gijirec/output");
    expect(payload).not.toContain("handwriting.md");
    expect(payload).not.toContain("ai_transcription.md");
  });
});
