## Verdict
- VERDICT: GO

## Summary

`capture-audio-controls` の技術設計は 24 AC 全件をトレーサビリティ表でカバーし、Persistent References の契約・境界・ADR と整合している。Pass A（QA→Arch→Sec）で非キャプチャ時の store 更新挙動、STRIDE  threat model、統合テスト観点の 3 点を `design.md` に反映。Pass B の反映検証・8 ドメイン監査・フェーズゲートはすべて合格。

## Reviewed Scope
- Reviewed contract paths: `docs/contracts/capture-audio-controls.md`, `docs/contracts/audio-capture-status.md`, `docs/contracts/audio-device-selection.md` (reference), `docs/contracts/audio-capture-pcm.md` (reference)
- ADR paths: `docs/architecture/adr/ADR-0014-capture-audio-controls-ingest-boundary.md`
- Contract sync: OK

## Findings

| ID | 重大度 | Pass | 内容 | 処置 |
| --- | --- | --- | --- | --- |
| QA-1 | Minor | QA | 非 `capturing` 時の invoke がセッション store を更新するか、ingest live apply の境界が不明確 | `CaptureAudioControlsService` に store 更新 vs live apply の分離を明記（反映済み） |
| QA-2 | Minor | QA | 非キャプチャ時の store 永続（次回 capturing 反映）の統合テストが未記載 | Integration Tests に観点追加（反映済み） |
| Sec-1 | Minor | Sec | requirements-review で委譲された IPC threat model が設計本文に STRIDE 表として未整形 | `Security Considerations` に threat model 表を追加（反映済み） |

## Decisions

- **QA**: 非 `capturing` 時は UI を disabled にしつつ、invoke 成功時はセッション store を更新する（`audio-device-selection` 同型）。ingest パスへの live apply とメーター emit は `capturing` 時のみ。
- **QA**: mic OFF + system 無効は `TRANSCRIBE_INGEST_NO_AUDIO_SOURCE`。system が接続済みだが無音の場合はエラーにしない（要件 1.5 の「利用可能な音声源」＝供給経路の有無）。
- **Arch**: ingest ゲイン所有は capture-audio-controls、`PcmIngestConsumer` は実装接点。`boundaries.md` の whisper-transcribe 節と矛盾なし。
- **Arch**: Extension — (1) ディスク永続化は Non-Goal。追加時は `CaptureAudioControlsStore` + 契約のみで吸収可能、PCM 契約変更不要。(2) cpal バージョン変更は device enumeration 側の影響。本 feature の mic gate / ingest gain / メーターは変更なしで維持可能。
- **Sec**: AuthN/AuthZ はローカル単一ユーザーで N/A。gain tampering は clamp + capability で軽減。dBFS メタデータのみ配信は accepted risk（音声内容は漏洩しない）。
- **Sec**: 再キャプチャ中の競合は `Mutex` 単一 writer で直列化。失敗時は既存 capture エラーフローに委譲。

## Reflected Fixes
| Finding | 対象セクション | 修正概要 | Pass |
| ------- | -------------- | -------- | ---- |
| QA-1 | Components → CaptureAudioControlsService | 非 capturing 時の store 更新と live apply 分離を Responsibilities に追記 | QA |
| QA-2 | Testing Strategy → Integration Tests | 非 capturing store → 次回 capturing 反映の統合テスト観点を追加 | QA |
| Sec-1 | Security Considerations | STRIDE threat model 表（5 行）を追加 | Sec |

## Specialist Summaries

### QA
異常系（音声源なし、非キャプチャ UI 無効、リソース圧迫時メーター低速化、ゲイン境界、再キャプチャ状態保持）を設計でカバー。派生エッジケースとして非 capturing invoke の store/live apply 境界を具体化。全 Unwanted Behavior AC（1.5, 1.6, 2.4, 3.4, 3.5, 5.3）に設計応答あり。

| Unwanted Behavior AC | 設計カバレッジ |
| --- | --- |
| 1.5 音声源なし | `TRANSCRIBE_INGEST_NO_AUDIO_SOURCE`（Service + contract） |
| 1.6 非キャプチャ時無効 | `capturePhase !== 'capturing'` → disabled |
| 2.4 非供給時非活性 | `ingest_level: null`、emit 停止 |
| 3.4 クリッピング防止 | 0.25–4.0 + soft limit 0.95 + UI hint |
| 3.5 非キャプチャ時ゲイン無効 | 同上 phase gate |
| 5.3 段階的 degrade | メーター 2 s まで延長、転写継続 |

### Arch
責務分割（mic gate / ingest gain / meter / UI / service）が明確。レイヤ依存は steering 準拠。Persistent References の `Mode: modify` 3 件（契約 2 + boundaries）はすべて存在し設計と一致。ADR-0014 が ingest 境界判断を記録。Anti-pattern スキャン: god object・循環依存・所有権競合なし。

### Sec
信頼境界は requirements スコープ境界および `audio-device-selection` 同等。新外部依存なし。Observability で PCM・デバイス名非出力を明記。STRIDE 表を設計に追加し requirements-review の Sec deferred 項目を解消。

## Gap-Domain Audit

| # | ドメイン | 結果 | 備考 |
| --- | --- | --- | --- |
| 1 | Requirements traceability | pass | 24 AC 全件が Traceability 表にマップ（Evidence に完全マトリクス） |
| 2 | Non-functional (non-security) | pass | 要件 5 を Operational Readiness + IngestLevelEmitter degrade でカバー |
| 3 | Observability | pass | Logging / Metrics / Debuggability あり。PII マスキング規則明記 |
| 4 | Operability | pass | 単一リリース、rollback 方針、永続化 N/A |
| 5 | Testability | pass | Unit / Integration / E2E / Performance 各層にシームと観点あり |
| 6 | Compatibility | pass | 新 IPC は additive。未調整時 ×1.25 で `transcribe-volume-normalize` 後方互換 |
| 7 | Scope fitness | pass | complexity_tier M、390 行は要件 5 NFR・契約参照に比例。gold-plating なし |
| 8 | Internal & external consistency | pass | 契約・boundaries・ADR・steering・requirements-review と矛盾なし。Contract sync: OK |

## 承認ゲートサマリ

### 検証済み観点
- Pass A QA / Arch / Sec 完了。Reflected Fixes 3 件を `design.md` で機械確認済み
- 反映検証：Pass 間の矛盾なし
- ギャップドメイン 1–8：pass 8 件
- Extension simulation 2 件（永続化追加・cpal 変更）いずれも境界内で吸収可能
- フェーズゲート 4 チェックすべて合格

### 自己修復した事項
- Pass B による `design.md` 追加修正なし（Pass A で完結）

### 受容が必要な残リスク
- **再キャプチャ中の競合**: `Mutex` 直列化で軽減するが、極端なタイミングで apply と restart が交差した場合の E2E 確認は実装時の統合テストで検証する。
- **ソフトリミット後の歪み**: 上限 4.0 でも極端入力では歪みうる（research で accepted）。実機で −18〜−17 dBFS 調整が十分かは手動確認。
- **tech.md の固定定数記述**: steering は `TRANSCRIBE_INGEST_GAIN` 固定を記載。実装完了後の steering 同期が必要（本設計フェーズでは ADR-0014 / 契約が正本）。

### 人間判断が必要な未決事項
- 0 件

## Evidence

参照ファイル:
- `docs/specs/capture-audio-controls/spec.json` — pass（`approvals.design.generated: true`、`phase: design-generated`）
- `docs/specs/capture-audio-controls/requirements.md` — pass（5 要件・24 AC）
- `docs/specs/capture-audio-controls/design.md` — pass（Pass A 修正後）
- `docs/specs/capture-audio-controls/research.md` — pass
- `docs/specs/capture-audio-controls/reviews/requirements-review.md` — pass（`VERDICT: GO`、`Phase Gate STATUS: VERIFIED`）
- `docs/steering/tech.md` — pass（レイヤ・ingest パターン整合）
- `docs/steering/structure.md` — pass（IPC ミラー・DeviceSelectorPanel パターン）
- `docs/steering/roadmap.md` — pass（`capture-audio-controls` 計画、deps: none）

### 要件 → 設計トレーサビリティマトリクス（完全）

| AC | 設計要素 |
| --- | --- |
| 1.1 | D-CaptureAudioControlsRow, set_capture_audio_controls |
| 1.2 | D-CaptureProcessingGate, mic_ingest_enabled |
| 1.3 | D-CaptureProcessingGate, 既定 true + 既存 mixer |
| 1.4 | D-CaptureAudioControlsService 即時 apply |
| 1.5 | D-CaptureAudioControlsService, TRANSCRIBE_INGEST_NO_AUDIO_SOURCE |
| 1.6 | D-CaptureAudioControlsRow, useCaptureStatus phase gate |
| 2.1 | D-IngestLevelEmitter, ingest-level event |
| 2.2 | D-IngestLevelEmitter, 1 s 窓集約 |
| 2.3 | D-CaptureAudioControlsRow, 「dBFS」ラベル |
| 2.4 | D-CaptureAudioControlsRow, ingest_level null |
| 2.5 | D-IngestLevelEmitter, メタデータのみ |
| 3.1 | D-CaptureAudioControlsRow, manual_ingest_gain slider |
| 3.2 | D-PcmIngestConsumer, set_ingest_gain |
| 3.3 | D-PcmIngestConsumer, ゲイン後 RMS |
| 3.4 | D-CaptureAudioControlsService, 0.25–4.0 + soft limit + UI hint |
| 3.5 | D-CaptureAudioControlsRow, disabled when non-capturing |
| 3.6 | D-CaptureAudioControlsStore, gain_user_adjusted + 既定 1.25 |
| 4.1 | D-DeviceSelectorPanel, 同一 section |
| 4.2 | D-CaptureAudioControlsStore, 非リセット |
| 4.3 | D-DeviceSelectorPanel, CaptureErrorDisplay 維持 |
| 4.4 | —（変更なし、Non-Goal 明記） |
| 4.5 | D-DeviceSelectorPanel, CSS flex 横並び |
| 5.1 | D-IngestLevelEmitter, 1 Hz のみ bus 非追加 |
| 5.2 | D-CaptureAudioControlsRow, 固定幅メーター |
| 5.3 | D-IngestLevelEmitter, メーター 2 s degrade |

### 反映検証（Pass B Step 1）
- QA-1 → `design.md` CaptureAudioControlsService Responsibilities 非 capturing 分離 — **verified**
- QA-2 → `design.md` Integration Tests 非 capturing 反映観点 — **verified**
- Sec-1 → `design.md` Security Considerations STRIDE 表 — **verified**

### Arch Extension Simulation

| シナリオ | 吸収コンポーネント | 契約影響 | レイヤ違反 |
| --- | --- | --- | --- |
| 制御状態のディスク永続化（Non-Goal からの将来追加） | CaptureAudioControlsStore, Service, contract | `capture-audio-controls.md` のみ | なし |
| cpal API 変更（デバイス列挙） | audio-device-selection 側。本 feature の gate/gain/meter は不変 | PCM 契約不変 | なし |

### Sec Threat Table（設計反映済み）

| # | Surface | Threat | Mitigation |
| --- | --- | --- | --- |
| 1 | set_capture_audio_controls | Tampering | clamp + INVALID_GAIN + capability |
| 2 | set_capture_audio_controls | Elevation | N/A ローカル単一ユーザー |
| 3 | ingest-level event | Info Disclosure | dBFS メタデータのみ |
| 4 | Observability | Info Disclosure | PCM/デバイス名非出力 |
| 5 | set_capture_audio_controls | DoS | ゲイン上限 + 1 Hz + Mutex |

## Phase Gate
- STATUS: VERIFIED
- CHECKS:
  1. `docs/specs/capture-audio-controls/design.md` 存在・設計内容あり — **pass**
  2. `spec.json` → `approvals.design.generated === true` — **pass**
  3. `reviews/design-review.md` → `VERDICT: GO` — **pass**（本レポート）
  4. Phase Gate `STATUS: VERIFIED` — **pass**（本レポート）
