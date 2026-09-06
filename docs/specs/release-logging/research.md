# Research & Design Decisions: release-logging

---
**Purpose**: brownfield ギャップ分析と Full discovery 成果を記録し、`design.md` の判断根拠とする。
---

## Summary

- **Feature**: `release-logging`
- **Discovery Scope**: Brownfield 拡張（Path D）+ Full discovery（`complexity_tier: L`）
- **Key Findings**:
  - 既存 observability は trait ディスパッチ + ホスト側 `Tracing*Observability` で構造化ログを出力済み。欠落は **subscriber のファイルレイヤ** と **運用者向け保存場所ドキュメント** のみ。
  - `init_tracing()` は `tracing_subscriber::fmt()` のコンソール出力のみ。release / dev の分岐なし。
  - マスキング（PCM・全文転写・デバイス表示名の除外）は各ドメイン observability 実装で既に遵守。永続化は subscriber 追加で同等イベントをそのままファイルへ流せる。
  - `tracing-appender` の non-blocking + `WorkerGuard` が Tauri ホストの起動時初期化パターンに適合。

## Gap Analysis（Step 2.0）

### Current State Investigation

| 領域 | 既存資産 | 備考 |
|------|----------|------|
| Capture observability | `gijirec-presentation/src/tauri/observability.rs` | trait `CaptureObservability`、phase / buffer drop / stream failure / RT latency |
| Transcribe observability | `gijirec-presentation/src/transcribe/observability.rs` | phase / PCM gap / block drop / inference latency / error |
| Editor observability | `gijirec-presentation/src/editor/observability.rs` | save started/completed（長さのみ）、settings updated |
| Host tracing backends | `src-tauri/src/{capture,transcribe,editor}_observability.rs` | `tracing::info/warn/error` + domain target |
| Subscriber 初期化 | `src-tauri/src/lib.rs::init_tracing()` | `EnvFilter` + console fmt のみ |
| 相関 ID | `init_session_id()` / `session_id()` | capture セッション ID を transcribe ログでも再利用 |
| 依存 | `tracing` 0.1, `tracing-subscriber` 0.3（env-filter） | `tracing-appender` 未導入 |
| 設定永続化パターン | Tauri `app.path().app_data_dir()` | editor-settings と同じ OS ユーザーデータ領域 |

**命名・レイヤ規約**: presentation crate は tracing マクロ禁止（cargo bylaw）。ログ出力はホスト crate の `Tracing*Observability` のみが担う。新規ドメインログは不要。

### Requirement-to-Asset Map

| 要件 AC | 既存 | ギャップ |
|---------|------|----------|
| 1.1 リリース永続化 | tracing イベント発火済み | release 向け file writer なし → **Missing** |
| 1.2 同一カテゴリ | 3 ドメイン observability 実装済み | subscriber 追加で充足 |
| 1.3 dev はファイル不要 | なし | release-only 分岐が必要 → **Missing** |
| 1.4 永続化失敗時も継続 | なし | 起動時 / セッション中の degrade 処理 → **Missing** |
| 2.1 運用ドキュメント | なし | operations 文書 → **Missing** |
| 2.2 app_data_dir 配下 | app_data_dir 利用実績あり | logs サブディレクトリ規約 → **Missing** |
| 2.3 セッション識別 | capture session_id のみ | 実行単位のログディレクトリ命名 → **Missing** |
| 2.4 最低 1 ファイル | なし | セッションログファイル作成 → **Missing** |
| 3.1–3.5 ドメインイベント | 各 observability フック実装済み | デフォルト INFO フィルタで充足（debug 除外） |
| 4.1–4.5 プライバシー | マスキング実装済み | 契約で禁止フィールドを明示化 |

### Implementation Approach Options

#### Option A: Extend `init_tracing()` only
- **Trade-offs**: 最小 diff。ホスト crate に file layer を追加。presentation 変更なし。
- **Risk**: Low — 既存パターンの延長。

#### Option B: New observability backends per domain
- **Trade-offs**: ドメインごとにファイル I/O。マスキング重複、bylaw 境界の混乱。
- **Rejected**: subscriber レイヤで一元化する方が単純。

#### Option C: Hybrid（採用）
- `init_tracing()` を Registry + layered fmt（release: file、dev: stdout）に拡張。
- 新規 `logging/` モジュールでパス解決・WorkerGuard 保持・失敗処理。
- 契約 `release-logging-persistence.md` で保存場所・禁止フィールドを正本化。
- operations 文書で取得手順を公開。

**Effort**: M（3–7 日）— 既存 tracing 基盤の拡張、新規 I/O 境界は限定的。  
**Risk**: Low — 確立済み tracing エコシステム、マスキングは上流で完結。

## Research Log

### tracing-appender による release ファイル永続化

- **Context**: リリースビルドでコンソールが無い。非ブロッキング I/O がキャプチャ RT パスを阻害しない必要がある。
- **Sources Consulted**: [tracing-appender docs](https://docs.rs/tracing-appender/latest/tracing_appender/), [tracing Discussion #2481](https://github.com/tokio-rs/tracing/discussions/2481)
- **Findings**:
  - `tracing_appender::non_blocking` + `RollingFileAppender::builder().rotation(Rotation::NEVER)` でセッション単一ファイルに適合。
  - `WorkerGuard` はプロセス寿命中ホストが保持必須（Tauri managed state または `run()` スコープ）。
  - キュー満杯時はログドロップ（backpressure なし）— 診断ログ用途では許容。失敗時は WARN を 1 回 emit。
  - dev + release 両方 console が必要な場合は layered subscriber（stdout + file）だが、要件 1.3 により **dev は stdout のみ**。
- **Implications**: release 判定は `cfg!(not(debug_assertions))` を正とする。`RUST_LOG` は dev / release 共通で EnvFilter に適用。

### ログ保存場所とセッション識別

- **Context**: Req 2.2–2.3。editor-settings と同じ app_data_dir を使い、権限昇格を避ける。
- **Findings**:
  - Tauri `app.path().app_data_dir()` は setup 後に解決可能。subscriber 初期化を setup 内に移すか、先に path を resolve してから `run()` 前に init する。
  - 実行セッション ID は `{UTC timestamp}-{capture session_id}` 形式でディレクトリ名に安全（ファイルシステム禁止文字を排除）。
  - パス: `{app_data_dir}/logs/sessions/{run_session_id}/gijirec.log`
  - 最新セッションへの symlink / pointer ファイル `logs/latest` は Windows 互換のため **テキストポインタ** `logs/latest-session.txt`（1 行に run_session_id）を採用。
- **Implications**: setup 順序変更が必要。永続化失敗時は stderr（存在すれば）+ tracing WARN で surface。

### 既存マスキングとの整合

- **Context**: Req 4。requirements-review で observability 実装が正本と判定済み。
- **Findings**:
  - Capture: `error_code` + abstract `port`（`mic` / `system`）のみ。デバイス表示名なし。
  - Transcribe: phase / error_code / session_id / counts / timing のみ。
  - Editor: `EditorSaveLogFields` は markdown 長のみ。単体テストで本文除外を検証済み。
  - 永persist 層はフィールドを追加しない — subscriber は上流イベントをそのままシリアライズ。
- **Implications**: 契約に禁止フィールド一覧を転記。新規ログ追加時は既存 observability フック経由を必須とする。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Subscriber レイヤ拡張 | Registry + fmt layer(s) | 既存 Tracing* 実装を再利用、bylaw 維持 | setup 順序・WorkerGuard 管理 | **採用** |
| ドメイン別ファイル | 各 observability が直接 write | 独立ローテーション | マスキング重複、3 ファイル混在 | 不採用 |
| 外部ログエージェント | filebeat 等 | 高機能 | スコープ外（クラウド/APM 禁止） | 不採用 |
| `log` crate 併用 | 別 logging ファサード | シンプル | 二重ログ系、構造化フィールド喪失 | 不採用 |

## Design Decisions

### Decision: release-only file layer via tracing-appender

- **Context**: Req 1.1, 1.3
- **Alternatives Considered**:
  1. 常時 file + console — dev でもファイル生成（要件 1.3 違反）
  2. カスタム RollingFile — tracing エコシステムから外れる
- **Selected Approach**: `cfg!(not(debug_assertions))` 時のみ non-blocking file layer を Registry に追加。debug ビルドは現行 stdout のみ。
- **Rationale**: 最小変更で既存イベントをそのまま永続化。presentation / infrastructure 無変更。
- **Trade-offs**: release ビルドでも `cargo tauri dev --release` 相当でファイルが出る — 意図どおり。
- **Follow-up**: integration test で release cfg 下のファイル作成を検証。

### Decision: セッション単位ディレクトリ + latest-session ポインタ

- **Context**: Req 2.3, 2.4
- **Selected Approach**: `{app_data_dir}/logs/sessions/{run_session_id}/gijirec.log` と `logs/latest-session.txt`。
- **Rationale**: 複数実行の切り分けが容易。OS 権限昇格不要。
- **Trade-offs**: ローテーション / 自動削除はスコープ外（requirements-review 受容済み）。

### Decision: 永続化失敗時の非ブロッキング degrade

- **Context**: Req 1.4
- **Selected Approach**: ディレクトリ作成 / appender 構築失敗時、stdout（あれば）に 1 行 diagnostic、tracing WARN `release_log_persistence_failed=true`、アプリ継続。
- **Rationale**: ユーザー機能（キャプチャ・文字起こし）を止めない。

## Synthesis Outcomes

- **Generalization**: 「release 診断ログ」は単一 subscriber 契約として横断的に扱い、ドメイン observability は変更しない。
- **Build vs Adopt**: `tracing-appender` を採用（Rust 標準 tracing エコシステム）。
- **Simplification**: ローテーション・リモート送信・UI 表示は実装しない。1 セッション 1 ファイルで開始。

## Risks & Mitigations

- **WorkerGuard 早期 drop** — Tauri managed `LogGuardState` で `run()` 寿命中保持。
- **ディスク満杯** — append 失敗は non-blocking 側で drop / error。WARN 1 回 + 機能継続（Req 1.4）。
- **共有端末のファイル読取** — OS ユーザーデータ ACL に依存。operations 文書で注意喚起。詳細 ACL 強化は将来検討。
- **setup 前 init** — app_data_dir 解決後に subscriber 構築。init 順序を design の System Flows で固定。

## References

- `src-tauri/src/lib.rs` — 現行 `init_tracing()`
- `src-tauri/crates/gijirec-presentation/src/{tauri,transcribe,editor}/observability.rs`
- `docs/specs/release-logging/requirements.md`
- [tracing-appender](https://docs.rs/tracing-appender/latest/tracing_appender/)
