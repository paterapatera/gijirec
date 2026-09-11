import { useCallback, useMemo, useRef, useState } from "react";
import {
  type AiTranscriptEditorSnapshotPort,
  createSaveOrchestrator,
  type HandwritingEditorSnapshotPort,
  type SaveOrchestrator,
} from "../../application/transcript/saveOrchestrator";
import {
  type SaveTranscriptSessionResult,
  saveTranscriptSession,
} from "../../infrastructure/tauri/editorCommands";
import {
  defaultInvoke,
  type InjectableInvokeFn,
} from "../../infrastructure/tauri/injectableInvoke";
import { showSaveResult } from "../components/SaveResultToast";
import type { EditorSettings } from "./editor-settings";

export interface UseSaveTranscriptOptions {
  handwritingEditor: HandwritingEditorSnapshotPort | null;
  aiEditor: AiTranscriptEditorSnapshotPort | null;
  settings: EditorSettings;
  sessionId: string;
  invokeFn?: InjectableInvokeFn;
  showSaveResultFn?: (result: SaveTranscriptSessionResult) => void;
}

export interface UseSaveTranscriptResult {
  onSave: () => Promise<SaveTranscriptSessionResult | undefined>;
  isSaving: boolean;
}

function invokeFailureResult(): SaveTranscriptSessionResult {
  return {
    success: false,
    error: {
      code: "INTERNAL",
      message_ja: "保存に失敗しました",
      action_ja: "保存先フォルダを選び直して、もう一度保存してください",
      recoverable: true,
    },
  };
}

/**
 * Wires SaveOrchestrator to Tauri save invoke and SaveResultToast feedback.
 * Snapshots editor content at save start; does not clear editors on failure.
 */
export function useSaveTranscript(options: UseSaveTranscriptOptions): UseSaveTranscriptResult {
  const {
    handwritingEditor,
    aiEditor,
    settings,
    sessionId,
    invokeFn = defaultInvoke,
    showSaveResultFn = showSaveResult,
  } = options;

  const orchestrator = useMemo(
    () =>
      createSaveOrchestrator({
        saveFn: (request) => saveTranscriptSession(request, { invokeFn }),
      }),
    [invokeFn],
  );

  const orchestratorRef = useRef<SaveOrchestrator>(orchestrator);
  orchestratorRef.current = orchestrator;

  const [isSaving, setIsSaving] = useState(false);

  const onSave = useCallback(async (): Promise<SaveTranscriptSessionResult | undefined> => {
    if (handwritingEditor === null || aiEditor === null) {
      return;
    }

    const isInitiator = !orchestratorRef.current.isSaving;
    if (isInitiator) {
      setIsSaving(true);
    }

    try {
      const result = await orchestratorRef.current.saveSession({
        handwritingEditor,
        aiEditor,
        settings,
        sessionId,
      });

      if (isInitiator) {
        showSaveResultFn(result);
      }

      return result;
    } catch {
      const result = invokeFailureResult();
      if (isInitiator) {
        showSaveResultFn(result);
      }
      return result;
    } finally {
      if (isInitiator) {
        setIsSaving(false);
      }
    }
  }, [aiEditor, handwritingEditor, sessionId, settings, showSaveResultFn]);

  return { onSave, isSaving };
}
