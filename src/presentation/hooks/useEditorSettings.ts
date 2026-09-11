import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import {
  getEditorSettings,
  pickSaveDirectory as pickSaveDirectoryCommand,
  setEditorSettings,
} from "../../infrastructure/tauri/editorCommands";
import {
  defaultInvoke,
  type InjectableInvokeFn,
} from "../../infrastructure/tauri/injectableInvoke";
import type { EditorSettings } from "./editor-settings";
import { DEFAULT_EDITOR_SETTINGS } from "./editor-settings";

export interface UseEditorSettingsOptions {
  invokeFn?: InjectableInvokeFn;
}

export interface UseEditorSettingsResult {
  settings: EditorSettings;
  isLoading: boolean;
  pickSaveDirectory: () => Promise<void>;
  setExportJsonlEnabled: (enabled: boolean) => Promise<void>;
}

async function loadSettings(
  invokeFn: InjectableInvokeFn,
  setSettings: (settings: EditorSettings) => void,
  setIsLoading: (loading: boolean) => void,
): Promise<void> {
  try {
    const loaded = await getEditorSettings({ invokeFn });
    setSettings(loaded);
  } catch {
    // Browser-only dev (no Tauri shell) — keep defaults.
  } finally {
    setIsLoading(false);
  }
}

/**
 * Loads and persists editor settings via Tauri IPC.
 * Restores on mount (`get_editor_settings`), updates via `set_editor_settings`
 * and `pick_save_directory` (pick does not auto-persist — this hook calls set after selection).
 */
export function useEditorSettings(options: UseEditorSettingsOptions = {}): UseEditorSettingsResult {
  const { invokeFn = defaultInvoke } = options;
  const [settings, setSettings] = useState<EditorSettings>(DEFAULT_EDITOR_SETTINGS);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    void loadSettings(invokeFn, setSettings, setIsLoading);
  }, [invokeFn]);

  const pickSaveDirectory = useCallback(async () => {
    try {
      const selected = await pickSaveDirectoryCommand({ invokeFn });
      if (selected === null) {
        return;
      }
      const updated = await setEditorSettings({ save_directory: selected }, { invokeFn });
      setSettings(updated);
    } catch {
      toast.error("保存先の設定に失敗しました", {
        description: "もう一度フォルダを選択してください",
      });
    }
  }, [invokeFn]);

  const setExportJsonlEnabled = useCallback(
    async (enabled: boolean) => {
      try {
        const updated = await setEditorSettings({ export_jsonl_enabled: enabled }, { invokeFn });
        setSettings(updated);
      } catch {
        toast.error("設定の保存に失敗しました", {
          description: "もう一度お試しください",
        });
      }
    },
    [invokeFn],
  );

  return {
    settings,
    isLoading,
    pickSaveDirectory,
    setExportJsonlEnabled,
  };
}
