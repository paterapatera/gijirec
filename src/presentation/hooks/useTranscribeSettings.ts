import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import {
  DEFAULT_TRANSCRIBE_SETTINGS,
  type GetTranscribeSettingsResponse,
  type LocalAvailability,
  type TranscribeSettings,
  type TranscribeSettingsUserError,
  type WhisperModelVariant,
  getTranscribeSettings,
  setTranscribeModelVariant,
} from "../../infrastructure/tauri/transcribeSettingsCommands";

export type { TranscribeSettings, WhisperModelVariant, LocalAvailability };

export interface UseTranscribeSettingsOptions {
  invokeFn?: typeof invoke;
}

export interface UseTranscribeSettingsResult {
  settings: TranscribeSettings;
  localAvailability: LocalAvailability;
  isLoading: boolean;
  setModelVariant: (variant: WhisperModelVariant) => Promise<void>;
}

const DEFAULT_LOCAL_AVAILABILITY: LocalAvailability = {
  q5_0: false,
  q8_0: false,
  fp16: false,
};

function isTranscribeSettingsUserError(error: unknown): error is TranscribeSettingsUserError {
  if (typeof error !== "object" || error === null) {
    return false;
  }
  return "code" in error && "message_ja" in error && "action_ja" in error;
}

async function loadTranscribeSettings(
  invokeFn: typeof invoke,
  setSettings: (settings: TranscribeSettings) => void,
  setLocalAvailability: (availability: LocalAvailability) => void,
  setIsLoading: (loading: boolean) => void,
): Promise<void> {
  try {
    const response: GetTranscribeSettingsResponse = await getTranscribeSettings({ invokeFn });
    setSettings(response.settings ?? DEFAULT_TRANSCRIBE_SETTINGS);
    setLocalAvailability(response.local_availability ?? DEFAULT_LOCAL_AVAILABILITY);
  } catch {
    setSettings(DEFAULT_TRANSCRIBE_SETTINGS);
    setLocalAvailability(DEFAULT_LOCAL_AVAILABILITY);
  } finally {
    setIsLoading(false);
  }
}

/**
 * Loads and updates whisper model variant settings via Tauri IPC.
 */
export function useTranscribeSettings(
  options: UseTranscribeSettingsOptions = {},
): UseTranscribeSettingsResult {
  const { invokeFn = invoke } = options;
  const [settings, setSettings] = useState<TranscribeSettings>(DEFAULT_TRANSCRIBE_SETTINGS);
  const [localAvailability, setLocalAvailability] =
    useState<LocalAvailability>(DEFAULT_LOCAL_AVAILABILITY);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    void loadTranscribeSettings(invokeFn, setSettings, setLocalAvailability, setIsLoading);
  }, [invokeFn]);

  const setModelVariant = useCallback(
    async (variant: WhisperModelVariant) => {
      try {
        const response = await setTranscribeModelVariant({ model_variant: variant }, { invokeFn });
        setSettings(response.settings ?? DEFAULT_TRANSCRIBE_SETTINGS);
      } catch (error) {
        const userError = isTranscribeSettingsUserError(error) ? error : null;
        toast.error(userError?.message_ja ?? "モデル設定の保存に失敗しました", {
          description: userError?.action_ja ?? "もう一度お試しください",
        });
      }
    },
    [invokeFn],
  );

  return {
    settings,
    localAvailability,
    isLoading,
    setModelVariant,
  };
}
