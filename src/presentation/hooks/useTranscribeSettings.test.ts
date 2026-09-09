import { afterEach, beforeAll, describe, expect, mock, test } from "bun:test";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import {
  DEFAULT_TRANSCRIBE_SETTINGS,
  type LocalAvailability,
  type TranscribeSettings,
} from "../../infrastructure/tauri/transcribeSettingsCommands";
import { setupTestDom } from "../../test-setup";
import { useTranscribeSettings } from "./useTranscribeSettings";

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

function createMockInvoke(
  options: { initial?: TranscribeSettings; localAvailability?: LocalAvailability } = {},
) {
  let persisted: TranscribeSettings = { ...(options.initial ?? DEFAULT_TRANSCRIBE_SETTINGS) };
  const localAvailability: LocalAvailability = options.localAvailability ?? {
    q5_0: false,
    q8_0: true,
    fp16: true,
  };
  const calls: InvokeCall[] = [];

  const invokeFn = async (cmd: string, args?: Record<string, unknown>) => {
    calls.push({ cmd, args });
    switch (cmd) {
      case "get_transcribe_settings":
        return { settings: { ...persisted }, local_availability: { ...localAvailability } };
      case "set_transcribe_model_variant":
        persisted = { model_variant: args?.model_variant as TranscribeSettings["model_variant"] };
        return { settings: { ...persisted } };
      default:
        throw new Error(`Unexpected command: ${cmd}`);
    }
  };

  return { invokeFn, calls, getPersisted: () => ({ ...persisted }) };
}

describe("useTranscribeSettings", () => {
  test("restores settings on mount", async () => {
    const mock = createMockInvoke({ initial: { model_variant: "q8_0" } });
    const { result } = renderHook(() => useTranscribeSettings({ invokeFn: mock.invokeFn }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.settings.model_variant).toBe("q8_0");
    expect(mock.calls[0]?.cmd).toBe("get_transcribe_settings");
  });

  test("setModelVariant syncs hook state with invoke response", async () => {
    const mock = createMockInvoke();
    const { result } = renderHook(() => useTranscribeSettings({ invokeFn: mock.invokeFn }));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    await act(async () => {
      await result.current.setModelVariant("q5_0");
    });

    expect(result.current.settings.model_variant).toBe("q5_0");
    expect(mock.getPersisted().model_variant).toBe("q5_0");
  });
});
