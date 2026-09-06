import { toast } from "sonner";
import type { SaveTranscriptSessionResult } from "../../infrastructure/tauri/editorCommands";

const SAVE_SUCCESS_TITLE = "保存しました";

/** Shows save success or failure feedback via Sonner toast. */
export function showSaveResult(result: SaveTranscriptSessionResult): void {
  if (result.success) {
    toast.success(SAVE_SUCCESS_TITLE, {
      description: result.output_directory,
    });
    return;
  }

  const error = result.error;
  if (error) {
    toast.error(error.message_ja, {
      description: error.action_ja,
    });
    return;
  }

  toast.error("保存に失敗しました", {
    description: "もう一度お試しください",
  });
}
