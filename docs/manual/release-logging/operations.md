# 運用手順: リリースビルド診断ログ

gijirec リリースビルド（配布用実行ファイル）の障害調査用ログの有効化・所在・収集方法。

## ログの有効化（必須）

**通常起動ではログは出力されません。** 障害調査時のみ、実行ファイル起動時に **`--log`** オプションを付けてください。

```text
# Windows（例）
gijirec.exe --log

# macOS（例）
./gijirec --log
```

- オプション未指定: ログファイルは作成されず、コンソール出力もありません。
- `--log` 指定: 下記の保存場所にセッション別ログが書き込まれます。
- `cargo tauri dev`（開発モード）では `--log` は無視され、コンソール出力のみです。

## 保存場所（`--log` 指定時）

| OS | `{app_data_dir}` の例 | ログディレクトリ |
|----|----------------------|------------------|
| Windows | `%APPDATA%\com.gijirec.app\`（Tauri identifier に依存） | `{app_data_dir}\logs\sessions\{run_session_id}\` |
| macOS | `~/Library/Application Support/com.gijirec.app/` | `{app_data_dir}/logs/sessions/{run_session_id}/` |

**ログファイル名**: `gijirec.log`（セッションディレクトリ内）

**最新セッションの特定**: `{app_data_dir}/logs/latest-session.txt` に 1 行で最新の `run_session_id` が記録される（`--log` で起動したセッションのみ）。該当 ID のサブディレクトリ内 `gijirec.log` を参照する。

## 収集手順

1. 問題を再現する際、**`--log` 付きで** gijirec を起動する。
2. 再現後、ウィンドウを閉じてセッションを終了する。
3. 上記 `{app_data_dir}` を開く。
4. `logs/latest-session.txt` を読み、`run_session_id` を確認する。
5. `logs/sessions/{run_session_id}/gijirec.log` をコピーして開発者・二次調査に提供する。
6. 特定セッションが分かっている場合は `latest-session.txt` を使わず、直接 `logs/sessions/{run_session_id}/` を指定してよい。

## ログに含まれないもの

プライバシー保護のため、以下はログに **記録されない**:

- 会議音声（PCM）
- 転写全文・手書き議事録本文
- マイク / スピーカーのデバイス表示名

含まれるのはフェーズ遷移、エラーコード、相関 ID、ドロップ件数、推論レイテンシ等の診断メトリクス。

## 開発モードとの違い

`cargo tauri dev` ではコンソール出力のみで、上記ファイルは **作成されない**（`--log` も無視）。

## トラブルシュート

| 症状 | 確認事項 |
|------|----------|
| `gijirec.log` が無い | **`--log` 付きで起動したか**確認。オプションなしではログは生成されない |
| ログが途中で止まる | ディスク容量・書き込み権限。アプリは動作継続する場合あり（永続化失敗は diagnostic 出力） |
| 古いセッションを調査したい | `logs/sessions/` 内のタイムスタンプ付きディレクトリ名で識別（`--log` 起動セッションのみ存在） |

契約詳細: `docs/contracts/release-logging-persistence.md`
