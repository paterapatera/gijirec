import {
  type AiTranscriptionJsonlRecord,
  toAiMarkdown,
  toJsonlRecords,
} from "../../domain/transcript/export";
import type { EditorSettings, TranscriptBlockView } from "../../domain/transcript/types";

/** transcript-editor-save.md / transcript-editor-status.md contract mirrors. */
interface EditorUserError {
  code: string;
  message_ja: string;
  action_ja: string;
  recoverable: boolean;
}

export interface SaveTranscriptSessionRequest {
  session_id: string;
  handwriting_markdown: string;
  ai_transcription_markdown: string;
  ai_transcription_jsonl?: AiTranscriptionJsonlRecord[];
}

export interface SaveTranscriptSessionResult {
  success: boolean;
  output_directory?: string;
  files_written?: string[];
  files_failed?: Array<{ path: string; reason_ja: string }>;
  error?: EditorUserError;
}

/** Snapshot port for handwriting editor plain-text export. */
export interface HandwritingEditorSnapshotPort {
  getPlainText(): string;
}

/** Snapshot port for AI transcript block export. */
export interface AiTranscriptEditorSnapshotPort {
  getBlocks(): TranscriptBlockView[];
}

type SaveTranscriptSessionFn = (
  request: SaveTranscriptSessionRequest,
) => Promise<SaveTranscriptSessionResult>;

export interface SaveOrchestratorDeps {
  saveFn: SaveTranscriptSessionFn;
}

interface SaveSessionInput {
  handwritingEditor: HandwritingEditorSnapshotPort;
  aiEditor: AiTranscriptEditorSnapshotPort;
  settings: EditorSettings;
  sessionId: string;
}

export interface SaveOrchestrator {
  readonly isSaving: boolean;
  saveSession(input: SaveSessionInput): Promise<SaveTranscriptSessionResult>;
}

function buildSaveRequest(
  input: SaveSessionInput,
  blocks: readonly TranscriptBlockView[],
): SaveTranscriptSessionRequest {
  const request: SaveTranscriptSessionRequest = {
    session_id: input.sessionId,
    handwriting_markdown: input.handwritingEditor.getPlainText(),
    ai_transcription_markdown: toAiMarkdown(blocks),
  };

  if (input.settings.export_jsonl_enabled) {
    request.ai_transcription_jsonl = toJsonlRecords(blocks);
  }

  return request;
}

export function createSaveOrchestrator(deps: SaveOrchestratorDeps): SaveOrchestrator {
  let inFlightSave: Promise<SaveTranscriptSessionResult> | null = null;
  let saving = false;

  return {
    get isSaving() {
      return saving;
    },

    saveSession(input: SaveSessionInput): Promise<SaveTranscriptSessionResult> {
      if (inFlightSave !== null) {
        return inFlightSave;
      }

      const blocks = [...input.aiEditor.getBlocks()];
      const request = buildSaveRequest(input, blocks);

      saving = true;
      inFlightSave = deps.saveFn(request).finally(() => {
        saving = false;
        inFlightSave = null;
      });

      return inFlightSave;
    },
  };
}
