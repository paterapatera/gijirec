import { listen } from "@tauri-apps/api/event";
import { type Ref, useCallback, useEffect, useRef } from "react";
import { Separator } from "@/components/ui/separator";
import type { SaveTranscriptSessionResult } from "../../infrastructure/tauri/editorCommands";
import type { EditorSettings } from "../hooks/editor-settings";
import type { TranscribeEventListenFn } from "../hooks/transcribe-status";
import { TRANSCRIBE_ERROR_EVENT } from "../hooks/transcribe-status";
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
 * Block subscription is localized in AiTranscriptPanel; retains editor content on transcribe errors.
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

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void resolvedListenFn(TRANSCRIBE_ERROR_EVENT, () => {
      // Requirement 9.3: retain editor content; do not clear on upstream error.
    }).then((handle) => {
      if (cancelled) {
        handle();
        return;
      }
      unlisten = handle;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [resolvedListenFn]);

  return (
    <div
      className="transcript-editor-view flex h-full flex-col"
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
      <HandwritingEditor ref={mergedHandwritingRef} />
      <Separator data-testid="transcript-editor-separator" />
      <AiTranscriptPanel
        listenFn={resolvedListenFn}
        {...(aiTranscriptEditorRef !== undefined ? { aiTranscriptEditorRef } : {})}
      />
    </div>
  );
}
