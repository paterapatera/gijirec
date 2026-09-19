import { listen } from "@tauri-apps/api/event";
import { type Ref, useCallback, useRef } from "react";
import { Separator } from "@/components/ui/separator";
import type { SaveTranscriptSessionResult } from "../../infrastructure/tauri/editorCommands";
import type { EditorSettings } from "../hooks/editor-settings";
import type { TranscribeEventListenFn } from "../hooks/transcribe-status";
import type { TranscriptBlockEventListenFn } from "../hooks/transcript-blocks";
import type { AiTranscriptEditorRef } from "./AiTranscriptEditor";
import { AiTranscriptPanel } from "./AiTranscriptPanel";
import { EditorToolbar } from "./EditorToolbar";
import { HandwritingEditor, type HandwritingEditorRef } from "./HandwritingEditor";

export interface TranscriptEditorViewProps {
  readonly onSave: () => Promise<SaveTranscriptSessionResult | undefined>;
  readonly isSaving: boolean;
  readonly settings: EditorSettings;
  readonly isLoading: boolean;
  readonly pickSaveDirectory: () => Promise<void>;
  readonly setExportJsonlEnabled: (enabled: boolean) => Promise<void>;
  readonly listenFn?: TranscriptBlockEventListenFn & TranscribeEventListenFn;
  readonly handwritingEditorRef?: Ref<HandwritingEditorRef>;
  readonly aiTranscriptEditorRef?: Ref<AiTranscriptEditorRef>;
}

/**
 * Root layout for dual transcript editors (handwriting + AI) with toolbar.
 * Block subscription is localized in AiTranscriptPanel; editors retain content on transcribe errors (req 9.3).
 */
export function TranscriptEditorView({
  onSave,
  isSaving,
  settings,
  isLoading,
  pickSaveDirectory,
  setExportJsonlEnabled,
  listenFn,
  handwritingEditorRef,
  aiTranscriptEditorRef,
}: TranscriptEditorViewProps) {
  const resolvedListenFn = listenFn ?? listen;

  const internalHandwritingRef = useRef<HandwritingEditorRef>(null);
  const externalHandwritingRef = useRef(handwritingEditorRef);
  externalHandwritingRef.current = handwritingEditorRef;

  const mergedHandwritingRef = useCallback((value: HandwritingEditorRef | null) => {
    internalHandwritingRef.current = value;
    const external = externalHandwritingRef.current;
    if (external === undefined || external === null) {
      return;
    }
    if (typeof external === "function") {
      external(value);
    } else {
      external.current = value;
    }
  }, []);

  return (
    <div
      className="transcript-editor-view flex h-full min-h-0 flex-1 flex-col"
      data-testid="transcript-editor-view"
    >
      <EditorToolbar
        onSave={onSave}
        isSaving={isSaving}
        settings={settings}
        isLoading={isLoading}
        pickSaveDirectory={pickSaveDirectory}
        setExportJsonlEnabled={setExportJsonlEnabled}
      />
      <div className="flex flex-1 min-h-0 flex-row" data-testid="transcript-editor-body">
        <div
          className="min-h-0 min-w-0 flex-1 overflow-auto"
          data-testid="transcript-editor-pane-handwriting"
        >
          <HandwritingEditor ref={mergedHandwritingRef} />
        </div>
        <Separator orientation="vertical" data-testid="transcript-editor-separator" />
        <div
          className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden"
          data-testid="transcript-editor-pane-ai"
        >
          <AiTranscriptPanel
            listenFn={resolvedListenFn}
            {...(aiTranscriptEditorRef !== undefined ? { aiTranscriptEditorRef } : {})}
          />
        </div>
      </div>
    </div>
  );
}
