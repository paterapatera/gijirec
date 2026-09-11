import { afterEach, beforeAll, describe, expect, mock, test } from "bun:test";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { asInjectableInvokeFn } from "../../infrastructure/tauri/injectableInvoke";
import { setupTestDom } from "../../test-setup";
import type { EditorSettings } from "./editor-settings";
import { DEFAULT_EDITOR_SETTINGS } from "./editor-settings";
import { useEditorSettings } from "./useEditorSettings";

mock.module("sonner", () => ({
  toast: {
    success: mock(() => {}),
    error: mock(() => {}),
  },
}));

beforeAll(() => {
  setupTestDom();
});

afterEach(() => {
  cleanup();
});

type InvokeCall = { cmd: string; args?: Record<string, unknown> };

function createMockInvoke(options: { initial?: EditorSettings; pickResult?: string | null } = {}) {
  let persisted: EditorSettings = { ...(options.initial ?? DEFAULT_EDITOR_SETTINGS) };
  const calls: InvokeCall[] = [];
  let pickResult = options.pickResult ?? null;

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

describe("useEditorSettings", () => {
  test("starts with default settings while loading", () => {
    const invokeFn = () => new Promise<never>(() => {});
    const { result } = renderHook(() => useEditorSettings({ invokeFn }));

    expect(result.current.settings).toEqual(DEFAULT_EDITOR_SETTINGS);
    expect(result.current.isLoading).toBe(true);
  });

  test("loads persisted settings from get_editor_settings on mount", async () => {
    const persisted: EditorSettings = {
      save_directory: "/home/user/notes",
      export_jsonl_enabled: true,
    };
    const { invokeFn, calls } = createMockInvoke({ initial: persisted });
    const { result } = renderHook(() => useEditorSettings({ invokeFn }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.settings).toEqual(persisted);
    expect(calls.some((c) => c.cmd === "get_editor_settings")).toBe(true);
  });

  test("restores settings after remount (restart simulation)", async () => {
    const persisted: EditorSettings = {
      save_directory: "/data/transcripts",
      export_jsonl_enabled: false,
    };
    const mock = createMockInvoke({ initial: persisted });

    const first = renderHook(() => useEditorSettings({ invokeFn: mock.invokeFn }));
    await waitFor(() => {
      expect(first.result.current.isLoading).toBe(false);
    });
    expect(first.result.current.settings).toEqual(persisted);
    first.unmount();

    const second = renderHook(() => useEditorSettings({ invokeFn: mock.invokeFn }));
    await waitFor(() => {
      expect(second.result.current.isLoading).toBe(false);
    });
    expect(second.result.current.settings).toEqual(persisted);
  });

  test("partially updates export_jsonl_enabled via set_editor_settings", async () => {
    const { invokeFn, calls } = createMockInvoke();
    const { result } = renderHook(() => useEditorSettings({ invokeFn }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    await act(async () => {
      await result.current.setExportJsonlEnabled(true);
    });

    expect(result.current.settings.export_jsonl_enabled).toBe(true);
    expect(result.current.settings.save_directory).toBeNull();
    expect(calls).toContainEqual({
      cmd: "set_editor_settings",
      args: { export_jsonl_enabled: true },
    });
  });

  test("pickSaveDirectory persists selected path via set_editor_settings", async () => {
    const selectedPath = "/home/user/chosen-dir";
    const mock = createMockInvoke({ pickResult: selectedPath });
    const { result } = renderHook(() => useEditorSettings({ invokeFn: mock.invokeFn }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    await act(async () => {
      await result.current.pickSaveDirectory();
    });

    expect(result.current.settings.save_directory).toBe(selectedPath);
    expect(mock.calls.some((c) => c.cmd === "pick_save_directory")).toBe(true);
    expect(mock.calls).toContainEqual({
      cmd: "set_editor_settings",
      args: { save_directory: selectedPath },
    });
    expect(mock.getPersisted().save_directory).toBe(selectedPath);
  });

  test("pickSaveDirectory does not persist when dialog is cancelled", async () => {
    const mock = createMockInvoke({ pickResult: null });
    const { result } = renderHook(() => useEditorSettings({ invokeFn: mock.invokeFn }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    await act(async () => {
      await result.current.pickSaveDirectory();
    });

    expect(result.current.settings.save_directory).toBeNull();
    expect(mock.calls.some((c) => c.cmd === "pick_save_directory")).toBe(true);
    expect(mock.calls.some((c) => c.cmd === "set_editor_settings")).toBe(false);
  });

  test("keeps previous settings when set_editor_settings rejects after pick", async () => {
    const selectedPath = "/home/user/chosen-dir";
    const mock = createMockInvoke({ pickResult: selectedPath });
    const invokeFn = asInjectableInvokeFn(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "set_editor_settings") {
        throw new Error("invalid args");
      }
      return mock.invokeFn(cmd, args);
    });
    const { result } = renderHook(() => useEditorSettings({ invokeFn }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    await act(async () => {
      await result.current.pickSaveDirectory();
    });

    expect(result.current.settings.save_directory).toBeNull();
  });
});
