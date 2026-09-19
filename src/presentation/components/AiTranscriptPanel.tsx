import type { Ref } from "react";
import type { TranscriptBlockEventListenFn } from "../hooks/transcript-blocks";
import { useTranscriptBlocks } from "../hooks/useTranscriptBlocks";
import { AiTranscriptEditor, type AiTranscriptEditorRef } from "./AiTranscriptEditor";

export interface AiTranscriptPanelProps {
  readonly listenFn: TranscriptBlockEventListenFn;
  readonly aiTranscriptEditorRef?: Ref<AiTranscriptEditorRef>;
}

/**
 * Localizes block-appended subscription so parent layout does not re-render on AI updates.
 */
export function AiTranscriptPanel({ listenFn, aiTranscriptEditorRef }: AiTranscriptPanelProps) {
  const session = useTranscriptBlocks({ listenFn });

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col">
      <AiTranscriptEditor ref={aiTranscriptEditorRef} blocks={session.blocks} />
    </div>
  );
}
