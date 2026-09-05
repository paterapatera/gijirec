## Verdict
- VERDICT: GO

## Summary

audio-capture の設計は要件 28 AC・Persistent References 契約・ADR と整合し、レイヤード境界・セキュリティ制約・非機能テスト計画を網羅している。QA/Arch/Sec の指摘 6 件を `design.md` に反映済み。Contract sync OK。Phase Gate VERIFIED。

## Reviewed Scope
- Reviewed contract paths: `docs/contracts/audio-capture-pcm.md`, `docs/contracts/audio-capture-status.md`
- ADR paths: `docs/architecture/adr/ADR-0001-platform-audio-capture.md`, `docs/architecture/adr/ADR-0002-bun-frontend-toolchain.md`
- Contract sync: OK

## Findings

| ID | 重大度 | 内容 | 対応 |
| ---- | ------ | ---- | ---- |
| QA-1 | Major | システム音声失敗時のマイクロールバックが曖昧（5.2 部分開始リスク） | CaptureOrchestrator に即時クローズを明記 |
| QA-2 | Major | `start()` / `stop()` の冪等性未定義（二重終了フック） | 冪等ルールを追加 |
| QA-3 | Major | `starting` 中のウィンドウ閉鎖遷移が stateDiagram に欠落 | `starting → stopping` を追加 |
| QA-4 | Major | 下流 consumer 遅延時のバックプレッシャー未定義（リソース枯渇） | バウンドキュー＋ドロップ方針を追加 |
| Arch-1 | Minor | Domain Model の PcmChunk フィールドが契約より不足 | 契約全フィールドを列挙 |
| Sec-1 | Major | 外部 crate の供給チェーン対策が Security Considerations に未記載 | Cargo.lock ピン留め・audit を追記 |
| Arch-2 | Minor | steering `tech.md` が npm/Node 記述のまま（ADR-0002 と乖離） | 許容 — ADR-0002 が正本。steering 更新は別タスク |

## Decisions

- **部分開始ロールバック**: マイク先行オープン後にシステム音声が失敗した場合、マイクを即クローズし `capturing` へ遷移しない（QA-1、要件 5.2 整合）。
- **冪等ライフサイクル**: Tauri の `CloseRequested` と `RunEvent::Exit` 二重発火を想定し `start`/`stop` を冪等化（QA-2）。
- **バックプレッシャー**: 下流遅延時は最大 3 チャンク（≈300 ms）を超えた分をドロップしメトリクス記録。リアルタイム会議キャプチャではデッドロック回避を優先（QA-4）。
- **供給チェーン**: cpal / screencapturekit / rubato / rtrb は `Cargo.lock` 固定＋`cargo audit`（Sec-1）。
- **steering 乖離**: Bun 採用は ADR-0002 と要件 8 が正本。`tech.md` の npm 記述は後続 steering 更新で解消（Arch-2、設計ブロッカーではない）。
- **ホットプラグ**: v1 は再起動案内のみ。自動再登録はスコープ外（設計 Risks 既記載、受容）。

## Reflected Fixes

| Finding | 対象セクション | 修正概要 | Pass |
| ------- | -------------- | -------- | ---- |
| QA-1 | D-CaptureOrchestrator Responsibilities | システム音声失敗時のマイクロールバックを明記 | QA |
| QA-2 | D-CaptureOrchestrator Responsibilities | start/stop 冪等ルールを追加 | QA |
| QA-3 | キャプチャライフサイクル stateDiagram | starting→stopping 遷移を追加 | QA |
| QA-4 | D-PcmChunkBus Implementation Notes | バウンドキュー＋ドロップ方針を追加 | QA |
| Sec-1 | Security Considerations | Cargo.lock ピン留め・cargo audit を追記 | Sec |
| Arch-1 | Domain Model PcmChunk | 契約全フィールドを列挙 | Arch |

## Specialist Summaries

### QA

- 要件 5（異常系）を Error Handling・stateDiagram・契約コードにマッピング済みを確認。
- 派生エッジケース: 部分開始ロールバック、冪等 stop、starting 中終了、下流遅延を検出し 4 件を設計に反映。
- Testing Strategy が異常系（5.2 単独継続禁止、権限拒否、バックプレッシャー）をカバー。
- 並行性: RT コールバックは rtrb 単一ライター、処理スレッド分離 — pass。

### Arch

- レイヤ依存は steering `structure.md` / cargo bylaw 方針と一致。presentation が composition root。
- Anti-pattern スキャン: god object・循環依存・漏洩抽象・共有 mutable 無秩序 — いずれも pass。
- Contract sync: `audio-capture-pcm.md`, `audio-capture-status.md`, `boundaries.md` は設計境界と整合。DRIFT なし。
- 拡張シミュレーション:
  - **whisper-transcribe 追加**: `PcmChunkBus` 登録点のみ変更、契約消費者側は `PcmChunkConsumer` 実装 — pass。
  - **cpal メジャー更新**: `MicCaptureAdapter` / `WindowsLoopbackAdapter` に閉じ込め、domain/application 変更不要 — pass。
- ADR-0001/0002 が重要技術判断をカバー。新規 ADR 不要。

### Sec

- 音声は sensitive。ネットワーク送信なし・PCM を Tauri イベントで露出しない（7.2）— 設計と契約で二重固定。
- Observability: PCM/音声バイト列をログに出さない、デバイス名は DEBUG 限定 — pass。
- AuthN/AuthZ N/A（7.4）。DoS はローカル単一ユーザー前提で rate limit N/A（受容）。
- 脅威テーブル全行に設計上の緩和または受容リスクを紐付け — pass。
- 供給チェーン対策を Security Considerations に追記済み。

## Gap-Domain Audit

| # | ドメイン | 結果 |
| - | -------- | ---- |
| 1 | Requirements traceability | pass — 28/28 AC マッピング（Evidence 参照） |
| 2 | Non-functional (non-security) | pass — Operational Readiness + Performance/Load テスト計画（4.2） |
| 3 | Observability | pass — logging/metrics/debuggability、PII マスキング規則あり |
| 4 | Operability | pass — Deployment/Rollback/Migration（N/A 明示） |
| 5 | Testability | pass — 全コンポーネントに trait/境界、テスト戦略で AC 検証可能 |
| 6 | Compatibility | pass — 契約 v1 初版、Changelog あり、下流 revalidation trigger 明記 |
| 7 | Scope fitness | pass — complexity L、要件外の gold-plating なし |
| 8 | Internal & external consistency | pass — 契約・ADR・diagram・prose 整合、Contract sync OK |

## 承認ゲートサマリ

### 検証済み観点

- QA/Arch/Sec Pass A 完了、Reflected Fixes 6 件を design.md で機械確認済み
- Gap-Domain 8/8 pass
- 要件 28 AC 全件トレーサビリティ確認
- Persistent References 契約 2 件 + boundaries + ADR 2 件読了、Contract sync OK

### 自己修復した事項

- Pass A/B で design.md に 6 件反映（上表 Reflected Fixes）
- Integration Tests にバックプレッシャー検証ケースを 1 件追加

### 受容が必要な残リスク

- **ホットプラグ自動復旧なし**: デバイス切断は再起動案内のみ（5.3 は満たすが UX は最小）。v1 受容。
- **性能数値の実機検証**: CPU < 5% 等は設計テスト計画に記載。CI では手動/ignore テスト混在。実装後に検証。
- **steering tech.md の Bun 未反映**: ADR-0002 が正本。ドキュメント整合は steering 更新で後追い。

### 人間判断が必要な未決事項

- 0 件（上記残リスクは設計委譲・v1 スコープとして受容可能）

## Evidence

### Unwanted Behavior AC → Design Coverage

| AC | 設計カバレッジ | 結果 |
| ---- | ------------- | ---- |
| 5.1 マイク不能 | CaptureOrchestrator error、MIC_* 契約コード | pass |
| 5.2 システム音声不能（フォールバック禁止） | 部分開始ロールバック、SYSTEM_* コード、契約禁止事項 | pass |
| 5.3 切断時安全停止 | DEVICE_DISCONNECTED、error 遷移、再起動案内 | pass |
| 5.4 行動可能な通知 | UserFacingError、action_ja 必須 | pass |

### Derived Edge Cases → Design Coverage

| エッジケース | 設計カバレッジ | 結果 |
| ------------ | ------------- | ---- |
| 部分開始（マイク OK → システム NG） | QA-1 ロールバック | pass（修正後） |
| 二重 stop / 二重 start | QA-2 冪等ルール | pass（修正後） |
| starting 中の早期終了 | QA-3 stateDiagram | pass（修正後） |
| 下流 consumer 遅延 | QA-4 バウンドキュー＋ドロップ | pass（修正後） |
| RT コールバックアロケーション | rtrb、allocation-free 境界 | pass |
| 片系統一時無音（capturing 中） | AudioMixer 50 ms 整列バッファ | pass |

### Requirements → Design Traceability (28 AC)

| AC | 設計要素 |
| ---- | -------- |
| 1.1 | D-MicCaptureAdapter, D-CaptureOrchestrator |
| 1.2 | D-WindowsLoopbackAdapter, D-MacScreenCaptureKitAdapter |
| 1.3 | D-CaptureOrchestrator 開始ゲート |
| 1.4 | ADR-0001, Non-Goals |
| 2.1 | D-AudioMixer |
| 2.2 | D-MonoResampler, D-ChunkEmitter, audio-capture-pcm |
| 2.3 | D-ChunkEmitter, D-PcmChunkBus, 100 ms 規約 |
| 2.4 | D-AudioMixer RMS gain |
| 3.1 | D-TauriLifecycleHook setup |
| 3.2 | D-TauriLifecycleHook CloseRequested |
| 3.3 | D-TauriLifecycleHook RunEvent::Exit |
| 3.4 | D-CaptureOrchestrator stop(), idle 状態 |
| 4.1 | D-*Adapter RT 制約, allocation-free |
| 4.2 | Performance/Load テスト計画, Operational Readiness |
| 5.1 | MIC_UNAVAILABLE / MIC_PERMISSION_DENIED |
| 5.2 | SYSTEM_* エラー、部分開始ロールバック |
| 5.3 | DEVICE_DISCONNECTED, 安全停止 |
| 5.4 | CaptureUserError.action_ja |
| 6.1 | D-MacScreenCaptureKitAdapter |
| 6.2 | D-WindowsLoopbackAdapter |
| 6.3 | D-TauriLifecycleHook Linux cfg ガード |
| 7.1 | preflight, OS 権限プロンプト |
| 7.2 | D-PcmChunkBus Rust 内部のみ |
| 7.3 | メモリバッファのみ、30 s 上限 |
| 7.4 | N/A — 認証なし明示 |
| 8.1 | package.json, tauri.conf, ADR-0002 |
| 8.2 | README Bun 手順 |
| 8.3 | README Bun 必須 |

### STRIDE Threat Table

| # | Surface | Threat (STRIDE) | Impact | Mitigation / Accepted risk |
| - | ------- | --------------- | ------ | -------------------------- |
| 1 | OS マイク/SCK | Spoofing / Elevation | 不正キャプチャ | OS 標準権限プロンプトのみ（7.1, Security Considerations） |
| 2 | PcmChunkBus | Information Disclosure | 会議音声漏洩 | Rust 内部バスのみ、Tauri PCM イベント非発行（7.2, 契約） |
| 3 | Error IPC イベント | Information Disclosure | 内部詳細露出 | UserFacingError マップ、INTERNAL は一般化（Error Handling） |
| 4 | tracing ログ | Information Disclosure | 音声 PII 漏洩 | PCM/サンプル非ログ、デバイス名 DEBUG 限定（Observability） |
| 5 | メモリバッファ | Denial of Service | メモリ枯渇 | 30 s リング上限、バックプレッシャードロップ（QA-4） |
| 6 | 下流 consumer | Denial of Service | デッドロック | バウンドキュー 3 チャンク、ドロップ＋メトリクス（QA-4） |
| 7 | 外部 crate | Tampering (supply chain) | 悪意ある依存 | Cargo.lock ピン留め、cargo audit（Sec-1） |
| 8 | ネットワーク | Information Disclosure | 外部送信 | 送信コードパス実装禁止（7.2, Non-Goals） |

### Arch Extension Simulation

| シナリオ | 吸収コンポーネント | 契約影響 | レイヤ違反 | 結果 |
| -------- | ----------------- | -------- | ---------- | ---- |
| whisper-transcribe consumer 追加 | D-PcmChunkBus 登録 | 消費側が PcmChunkConsumer 実装のみ | なし | pass |
| cpal API 変更 | infrastructure adapters | domain/application 不変 | なし | pass |

### Phase inputs

- `docs/specs/audio-capture/design.md` — pass（修正後）
- `docs/specs/audio-capture/requirements.md` — pass（approved）
- `docs/specs/audio-capture/spec.json` — `approvals.design.generated: true`
- `docs/steering/tech.md`, `structure.md`, `roadmap.md` — pass
- `docs/specs/audio-capture/reviews/requirements-review.md` — VERDICT GO

## Phase Gate

- STATUS: VERIFIED
- CHECKS:
  1. design.md exists with design content — pass
  2. spec.json approvals.design.generated === true — pass
  3. VERDICT: GO — pass
  4. Phase Gate STATUS: VERIFIED — pass
  5. approvals.design.approved === false — pass (pre-human-approval)
