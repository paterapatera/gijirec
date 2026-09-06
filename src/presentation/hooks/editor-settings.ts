/** Contract types per `docs/contracts/transcript-editor-settings.md`. */

export interface EditorSettings {
  /** 保存基点ディレクトリの絶対パス。未設定は null */
  save_directory: string | null;
  /** タイムスタンプ付き JSONL 出力の有効/無効 */
  export_jsonl_enabled: boolean;
}

export const DEFAULT_EDITOR_SETTINGS: EditorSettings = {
  save_directory: null,
  export_jsonl_enabled: false,
};
