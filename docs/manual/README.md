# 手動検証・運用ドキュメント

feature spec 完了後も残す、実機確認・性能計測・運用手順の正本。CI では実行しない項目のチェックリストと実行記録を置く。

| 領域 | ファイル | 内容 |
|------|----------|------|
| audio-capture | [e2e-checklist.md](./audio-capture/e2e-checklist.md) | 実機キャプチャ・権限 UI |
| audio-capture | [performance-results.md](./audio-capture/performance-results.md) | 30 分性能実測記録 |
| audio-capture | [manual-concurrency-checklist.md](./audio-capture/manual-concurrency-checklist.md) | Zoom / Teams 並走・マイク解放 |
| audio-device-selection | [performance-results.md](./audio-device-selection/performance-results.md) | 選択変更 → capturing 復帰 < 2 s |
| whisper-transcribe | [performance-results.md](./whisper-transcribe/performance-results.md) | 10 分転写・E2E 遅延実測 |
| transcript-editor | [validation-checklist.md](./transcript-editor/validation-checklist.md) | ブロック追記性能・保存・ログ除外 |
| release-logging | [operations.md](./release-logging/operations.md) | `--log` 有効化・ログ収集 |
| fix-release-transcribe | [smoke-checklist.md](./fix-release-transcribe/smoke-checklist.md) | release EXE 転写パリティ smoke |

手順の概要はルート [README.md](../../README.md) を参照。横断テスト方針は [docs/steering/testing.md](../steering/testing.md)。

---
_updated_at: 2026-09-07_
