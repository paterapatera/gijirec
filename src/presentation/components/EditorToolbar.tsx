import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import type { SaveTranscriptSessionResult } from "../../infrastructure/tauri/editorCommands";
import type { EditorSettings } from "../hooks/editor-settings";

export interface EditorToolbarProps {
  readonly onSave: () => Promise<SaveTranscriptSessionResult | undefined>;
  readonly isSaving: boolean;
  readonly settings: EditorSettings;
  readonly isLoading: boolean;
  readonly pickSaveDirectory: () => Promise<void>;
  readonly setExportJsonlEnabled: (enabled: boolean) => Promise<void>;
}

export function EditorToolbar({
  onSave,
  isSaving,
  settings,
  isLoading,
  pickSaveDirectory,
  setExportJsonlEnabled,
}: EditorToolbarProps) {
  const handleSave = () => {
    void onSave();
  };

  const handlePickDirectory = () => {
    void pickSaveDirectory();
  };

  const handleJsonlToggle = (checked: boolean) => {
    void setExportJsonlEnabled(checked);
  };

  return (
    <div
      className="editor-toolbar flex flex-wrap items-center gap-3 p-3"
      data-testid="editor-toolbar"
      style={{ backgroundColor: "var(--muted)" }}
    >
      <Button
        type="button"
        data-testid="save-button"
        disabled={isSaving || isLoading}
        onClick={handleSave}
      >
        {isSaving ? "保存中..." : "保存"}
      </Button>
      <Button
        type="button"
        variant="outline"
        data-testid="pick-directory-button"
        disabled={isLoading}
        onClick={handlePickDirectory}
      >
        保存先を選択
      </Button>
      <div className="flex items-center gap-2">
        <Switch
          id="export-jsonl-enabled"
          data-testid="export-jsonl-switch"
          checked={settings.export_jsonl_enabled}
          disabled={isLoading}
          onCheckedChange={handleJsonlToggle}
        />
        <Label htmlFor="export-jsonl-enabled">タイムスタンプ付き JSONL 出力</Label>
      </div>
    </div>
  );
}
