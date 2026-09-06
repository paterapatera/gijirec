import { listen } from "@tauri-apps/api/event";
import { type Ref, useEffect, useRef } from "react";
import { Separator } from "@/components/ui/separator";
import type { SaveTranscriptSessionResult } from "../../infrastructure/tauri/editorCommands";
import type { EditorSettings } from "../hooks/editor-settings";
import type { TranscribeEventListenFn } from "../hooks/transcribe-status";
import { TRANSCRIBE_ERROR_EVENT } from "../hooks/transcribe-status";
import type { TranscriptBlockEventListenFn } from "../hooks/transcript-blocks";
import { useTranscriptBlocks } from "../hooks/useTranscriptBlocks";
import { AiTranscriptEditor, type AiTranscriptEditorRef } from "./AiTranscriptEditor";
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

function mergeRefs<T>(...refs: Array<Ref<T> | undefined>): (value: T | null) => void {
  return (value) => {
    for (const ref of refs) {
      if (ref === undefined || ref === null) {
        continue;
      }
      if (typeof ref === "function") {
        ref(value);
      } else {
        ref.current = value;
      }
    }
  };
}

/**
 * Root layout for dual transcript editors (handwriting + AI) with toolbar.
 * Subscribes to upstream block-appended events and retains editor content on transcribe errors.
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
  const session = useTranscriptBlocks({ listenFn: resolvedListenFn });

  const internalHandwritingRef = useRef<HandwritingEditorRef>(null);

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
      <HandwritingEditor ref={mergeRefs(internalHandwritingRef, handwritingEditorRef)} />
      <Separator data-testid="transcript-editor-separator" />
      <AiTranscriptEditor ref={aiTranscriptEditorRef} blocks={session.blocks} />
    </div>
  );
}
