import { afterEach, beforeAll, describe, expect, test } from "bun:test";
import { act, cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import {
  type SaveTranscriptSessionResult,
  saveTranscriptSession,
} from "../../infrastructure/tauri/editorCommands";
import { asInjectableInvokeFn } from "../../infrastructure/tauri/injectableInvoke";
import { setupTestDom } from "../../test-setup";
import type { EditorSettings } from "../hooks/editor-settings";
import { DEFAULT_EDITOR_SETTINGS } from "../hooks/editor-settings";
import { useEditorSettings } from "../hooks/useEditorSettings";
import { EditorToolbar } from "./EditorToolbar";

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

type InvokeCall = { cmd: string; args?: Record<string, unknown> };

function createMockInvoke(
  options: {
    initial?: EditorSettings;
    pickResult?: string | null;
    saveResult?: SaveTranscriptSessionResult;
  } = {},
) {
  let persisted: EditorSettings = { ...(options.initial ?? DEFAULT_EDITOR_SETTINGS) };
  const calls: InvokeCall[] = [];
  let pickResult = options.pickResult ?? null;
  const saveResult: SaveTranscriptSessionResult = options.saveResult ?? {
    success: false,
    error: {
      code: "SAVE_DIRECTORY_NOT_SET",
      message_ja: "保存先が設定されていません",
      action_ja: "保存先フォルダを選択してください",
      recoverable: true,
    },
  };

  const invokeFn = async (cmd: string, args?: Record<string, unknown>) => {
    calls.push({ cmd, args });
    switch (cmd) {
      case "get_editor_settings":
        return { ...persisted };
      case "set_editor_settings":
        persisted = { ...persisted, ...args };
        return { ...persisted };
      case "pick_save_directory":
        return pickResult;
      case "save_transcript_session":
        return saveResult;
      default:
        throw new Error(`Unexpected command: ${cmd}`);
    }
  };

  return {
    invokeFn: asInjectableInvokeFn(invokeFn),
    calls,
    getPersisted: () => ({ ...persisted }),
    setPickResult: (path: string | null) => {
      pickResult = path;
    },
  };
}

async function waitForToolbarReady(getByTestId: (id: string) => HTMLElement): Promise<void> {
  await waitFor(() => {
    const button = getByTestId("pick-directory-button");
    expect(button.hasAttribute("disabled")).toBe(false);
  });
}

function renderToolbarWithSettings(
  mock: ReturnType<typeof createMockInvoke>,
  options: {
    isSaving?: boolean;
    onSave?: () => Promise<SaveTranscriptSessionResult | undefined>;
  } = {},
) {
  function Harness() {
    const { settings, isLoading, pickSaveDirectory, setExportJsonlEnabled } = useEditorSettings({
      invokeFn: mock.invokeFn,
    });

    return (
      <EditorToolbar
        onSave={options.onSave ?? (async () => {})}
        isSaving={options.isSaving ?? false}
        settings={settings}
        isLoading={isLoading}
        pickSaveDirectory={pickSaveDirectory}
        setExportJsonlEnabled={setExportJsonlEnabled}
      />
    );
  }

  return render(<Harness />);
}

describe("EditorToolbar", () => {
  test("pick save directory via parent-provided pickSaveDirectory", async () => {
    const selectedPath = "/home/user/transcripts";
    const mock = createMockInvoke({ pickResult: selectedPath });
    const { getByTestId } = renderToolbarWithSettings(mock);

    await waitForToolbarReady(getByTestId);

    await act(async () => {
      fireEvent.click(getByTestId("pick-directory-button"));
    });

    await waitFor(() => {
      expect(mock.calls.some((c) => c.cmd === "pick_save_directory")).toBe(true);
    });
    expect(mock.calls).toContainEqual({
      cmd: "set_editor_settings",
      args: { save_directory: selectedPath },
    });
    expect(mock.getPersisted().save_directory).toBe(selectedPath);
  });

  test("JSONL switch persists export_jsonl_enabled via set_editor_settings", async () => {
    const mock = createMockInvoke();
    const { getByTestId } = renderToolbarWithSettings(mock);

    await waitForToolbarReady(getByTestId);

    const switchEl = getByTestId("export-jsonl-switch");
    expect(switchEl.getAttribute("data-state")).toBe("unchecked");

    await act(async () => {
      fireEvent.click(switchEl);
    });

    await waitFor(() => {
      expect(switchEl.getAttribute("data-state")).toBe("checked");
    });
    expect(mock.calls).toContainEqual({
      cmd: "set_editor_settings",
      args: { export_jsonl_enabled: true },
    });
    expect(mock.getPersisted().export_jsonl_enabled).toBe(true);
  });

  test("save when save_directory is null returns SAVE_DIRECTORY_NOT_SET from invoke", async () => {
    const mock = createMockInvoke({
      initial: { save_directory: null, export_jsonl_enabled: false },
    });
    let saveResult: SaveTranscriptSessionResult | undefined;

    const onSave = async () => {
      saveResult = await saveTranscriptSession(
        {
          session_id: "test-session",
          handwriting_markdown: "",
          ai_transcription_markdown: "",
        },
        { invokeFn: mock.invokeFn },
      );
      return saveResult;
    };

    const { getByTestId } = renderToolbarWithSettings(mock, { onSave });

    await waitForToolbarReady(getByTestId);

    await act(async () => {
      fireEvent.click(getByTestId("save-button"));
    });

    await waitFor(() => {
      expect(saveResult).toBeDefined();
    });
    expect(mock.calls.some((c) => c.cmd === "save_transcript_session")).toBe(true);
    expect(saveResult?.error?.code).toBe("SAVE_DIRECTORY_NOT_SET");
    expect(saveResult?.success).toBe(false);
  });

  test("save button is disabled and shows loading while isSaving", async () => {
    const mock = createMockInvoke();
    const { getByTestId } = renderToolbarWithSettings(mock, { isSaving: true });

    const saveButton = getByTestId("save-button");
    expect(saveButton.hasAttribute("disabled")).toBe(true);
    expect(saveButton.textContent).toContain("保存中");
  });

  test("renders toolbar with muted background", async () => {
    const mock = createMockInvoke();
    const { getByTestId } = renderToolbarWithSettings(mock);
    const toolbar = getByTestId("editor-toolbar");
    expect(toolbar).toBeTruthy();
    expect((toolbar as HTMLElement).style.backgroundColor).toBe("var(--muted)");
  });
});
