# audio-capture E2E / UI チェックリスト

実機キャプチャ・権限ダイアログ・ウィンドウ閉鎖後のマイク解放（設計 E2E/UI 項目 1–3）。Playwright 等のブラウザ E2E フレームワークは未導入。項目 1–2 は React コンポーネントテストでカバーし、実機 Tauri 起動は半自動（手動確認）とする。

## 自動（項目 1–2）

| 設計項目 | 要件 | テスト | 内容 |
|---------|------|--------|------|
| E2E/UI 1 | 3.1 | `App.test.tsx` — `shows capturing phase after phase-changed event` | `capture-phase-changed` 受信後、UI に `capturing` が表示される |
| E2E/UI 2 | 5.4, 7.1 | `App.test.tsx` — `shows message_ja and prominent action_ja for permission denied` | `MIC_PERMISSION_DENIED` 相当のエラーで `message_ja` / `action_ja` を表示し、技術コードは UI に出さない |

### 実行

```bash
bun test src/presentation
```

フル品質ゲート（型・lint・アーキテクチャ含む）:

```bash
bun run check
```

## 手動（項目 1 の実機確認）

ライフサイクル結線経由で Tauri 起動時に `capturing` へ遷移することを目視確認する。

1. リポジトリルートで `cargo tauri dev --manifest-path src-tauri/Cargo.toml` を実行（Windows / macOS。Linux は非対応メッセージのみ）。
2. マイクおよび（macOS では）画面収録 / （Windows では）オーディオ出力デバイス権限を OS プロンプトで許可する。
3. メインウィンドウの状態表示が `capturing` になることを確認する（`data-testid="capture-phase"` 相当の表示）。

## 手動（項目 2 の実機確認・任意）

OS 設定でマイク権限を拒否した状態で起動し、UI に日本語の `action_ja` ガイダンスが表示され、`MIC_PERMISSION_DENIED` 等のコードが画面に出ないことを確認する。

## 手動（項目 3 — 会議アプリ並走・マイク解放）

Zoom / Teams 並走時の相手音声途切れと、ウィンドウ閉鎖後のマイクインジケータ消灯は **実機のみ** で確認する。

- **手順と実行記録:** [manual-concurrency-checklist.md](manual-concurrency-checklist.md)
- CI では **not executed** — 実機実施後に同ファイルの記録テンプレートを更新すること

### 合格の要点

| 設計項目 | 要件 | 確認内容 |
|---------|------|----------|
| E2E/UI 3 | 3.2 | ウィンドウ閉鎖後、プロセス終了かつ OS マイクインジケータ消灯 |
| Performance/Load 4 | 4.1 | Zoom / Teams 再生中、相手音声の **持続的** 途切れなし（gijirec 停止で改善する途切れは不合格） |
