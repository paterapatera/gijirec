# Security Standards

gijirec のセキュリティ姿勢。ローカルファーストのデスクトップアプリ向け（認証・クラウド API なし）。

## Philosophy

- **最小収集** — 会議音声は処理のためだけにメモリ上で扱い、不要なら残さない
- **ローカル完結** — モデル取得後はオフライン。キャプチャパイプラインから外部ネットワークへ音声を送らない
- **権限は明示** — OS が求めるマイク・画面収録権限を隠さず、`action_ja` でユーザーに誘導
- **ログは安全** — 会議内容・PCM 生データ・デバイス識別子の過剰露出を避ける
- **依存の健全性** — `bun run check` / `bun run rust:check` で既知の品質ゲートを維持

## Threat Model（v1 スコープ）

| 脅威 | 対策 |
|------|------|
| 音声の意図しない外部送信 | キャプチャ実装にネットワーク送信なし（boundaries 準拠） |
| ディスクへの音声残存 | v1 は PCM の意図的永続化なし。将来保存する spec では明示的オプトイン |
| 権限の過剰要求 | 必要な OS 権限のみ（マイク + macOS 画面収録/SCK） |
| ログ・クラッシュレポートからの漏洩 | PCM・転写テキストを tracing に出さない |
| サプライチェーン | Bun lock + Cargo.lock を正本。定期的な依存更新は別プロセス |

**スコープ外（v1）**: ユーザー認証、マルチテナント、リモート管理、暗号化されたクラウド同期。

## OS Permissions

| OS | 権限 | 用途 | ユーザー誘導 |
|----|------|------|--------------|
| 全 OS | マイク | 自分の発言キャプチャ | `MIC_PERMISSION_DENIED` → 設定パス in `action_ja` |
| macOS | 画面とシステムオーディオ録音 | ScreenCaptureKit でシステム音声 | `SYSTEM_AUDIO_PERMISSION_DENIED` |
| Windows | （ループバックは追加ダイアログなしのことが多い） | WASAPI loopback | デバイス・OS 確認を `action_ja` で案内 |

権限拒否時は **サイレントデグラデーション禁止** — マイクのみ続行などせず、契約どおり `error` phase で停止。

## Sensitive Data Handling

### 音声（PCM）

- メモリ上の `PcmChunk` は下流 consumer（将来 whisper）へのみ渡す
- Tauri イベントでフロントに PCM を送らない（`audio-capture-pcm.md`）
- テストでも observability 記録にサンプル配列を含めない

### 転写テキスト（将来）

- ローカルメモリ + ユーザー明示保存の Markdown のみ想定
- クラウド STT は product スコープ外

### ログ

**ログしてよい**:
- phase 遷移、error code、buffer drop カウント、correlation id
- 内部 `CaptureError::Internal { detail }`（開発者向け）

**ログしてはいけない**:
- PCM サンプル列、長い転写全文
- マイクデバイスのユーザー表示名（プライバシー・再識別リスク）
- API キー、モデルダウンロード URL に埋め込まれたトークン（将来導入時）

## Secrets & Configuration

- **リポジトリに秘密情報をコミットしない** — `.env`、API キー、個人トークン
- 将来のモデルダウンロード URL は環境変数またはビルド時注入を検討。平文をソースに直書きしない
- `RUST_LOG` は開発者が制御。本番相当ビルドのデフォルトは INFO 以下で十分

## Input Validation

- **契約境界**: `PcmChunk` の frame_count / sample_rate 制約を domain で検証
- **Tauri command**: 将来追加する command は presentation で入力を検証し、不正は typed error
- **フロント**: ユーザー入力は transcript-editor spec で扱う。v1 UI は表示のみで入力面は最小

## Dependency & Build

- フロント: `bun.lock` をロックファイルとして使用
- Rust: workspace `Cargo.lock` をコミット
- サードパーティ crate は OS API ラッパー（cpal, screencapturekit）に限定し、不要なネットワーク crate を domain に入れない

## Desktop Shell（Tauri）

- **IPC 表面を最小化** — 必要な command / event のみ公開
- **capabilities / permissions** — Tauri 2 の permission ファイルで command を明示許可
- **ウィンドウ閉鎖** — キャプチャ・マイクリソースを確実に解放（マイクインジケータ消灯は手動検証）

## Incident Response（軽量）

1. ログに会議内容が含まれた疑い → 該当リリースの logging 修正、ユーザーへ再現手順確認
2. 意図しないネットワーク通信 → boundaries 違反としてブロッキングバグ扱い
3. 依存脆弱性 → advisory に従い更新 PR（別途プロセス）

## Related

- 製品スコープ: `docs/steering/product.md`（オフライン・仮想デバイス不使用）
- 境界: `docs/architecture/boundaries.md`
- エラーログ規約: `docs/steering/error-handling.md`
- 契約（PCM 非送信）: `docs/contracts/audio-capture-pcm.md`

---
_updated_at: 2026-09-05_
_Focus on local-first desktop posture, not enterprise IAM patterns._
