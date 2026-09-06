## Verdict
- VERDICT: GO

## Summary

`audio-device-selection` の設計は要件 7 件・33 AC をトレース可能にカバーし、Persistent References の契約・ADR・`boundaries.md` と整合している。Pass A（QA→Arch→Sec）で 4 件の局所修正を `design.md` に反映し、Pass B の反映検証・8 ドメイン監査・設計フェーズゲートをすべて pass した。

## Reviewed Scope
- Reviewed contract paths: `docs/contracts/audio-device-selection.md`, `docs/contracts/audio-capture-status.md`, `docs/contracts/audio-capture-pcm.md`（reference）
- ADR paths: `docs/architecture/adr/ADR-0001-platform-audio-capture.md`, `docs/architecture/adr/ADR-0009-macos-speaker-selection-strategy.md`
- Contract sync: OK

## Findings

| ID | 重大度 | Pass | 内容 | 処置 |
| --- | --- | --- | --- | --- |
| QA-1 | Major | QA | 連続 `set_device_selection` / 再開中の競合で二重 `restart_with_selection` のリスク | `DeviceSelectionService` に直列化・最新選択のみ再開を追記（Reflected Fixes） |
| QA-2 | Minor | QA | 同一選択の再送信で不要な再キャプチャが起きうる | idempotent no-op を追記（Reflected Fixes） |
| QA-3 | Major | QA | `error` フェーズからの再選択回復経路が明示不足（要件 4.5） | `error` フェーズでも `restart_with_selection` を呼ぶ旨を追記（Reflected Fixes） |
| QA-4 | Minor | QA | 競合・回復の単体テストが Testing Strategy に未記載 | Unit Tests に 3 件追加（Reflected Fixes） |
| ARCH-1 | Minor | Arch | `MACOS_OUTPUT_NOT_DEFAULT` が invoke と capture イベントの二重表面 — 設計内の明示が薄い | 契約・シーケンス図で既に整合。Decisions で受容（ドリフトなし） |
| SEC-1 | Major | Sec | Security Considerations に trust boundary / 入力検証 / DoS 緩和が不足（requirements-review SEC-2 defer 先） | Trust Boundaries + Controls 小節を追加（Reflected Fixes） |
| SEC-2 | Minor | Sec | invoke レート制限なし | 単一利用者ローカルアプリとして Decisions で受容 |
| FINAL-1 | Minor | Final | 要件 5 AC3 の 2 s 上限は設計 NFR だが実機ばらつきリスク | 性能テスト + 手動確認を残リスクとして人間ゲートへ |

## Decisions

- **QA**: 選択変更は `DeviceSelectionService` で単一フライト直列化し、再開完了後に最新 `DeviceSelection` で 1 回だけ `restart_with_selection` を実行する。同一選択の再送信は no-op。
- **QA**: `error` フェーズでの `set_selection` は回復経路として許可し、要件 4.5（再選択可能状態）を満たす。
- **Arch**: ハイブリッド拡張（Option C）は steering レイヤ・既存 `CaptureOrchestrator` 再利用と一致。`DeviceSelectionService` が選択ドメインを application に隔離し、PCM 所有は audio-capture に留まる。
- **Arch**: `MACOS_OUTPUT_NOT_DEFAULT` の二重表面（`audio-device-selection` invoke / `audio-capture-status` event）は ADR-0009 の preflight（選択時）と capture 時検証の意図的分離。契約正本と矛盾なし。
- **Sec**: 認証・認可 N/A（要件 7 AC4）。デバイス名は潜在 PII — `INFO` は ID のみ、名前は `DEBUG` 限定（Observability + Security Controls）。
- **Sec**: invoke レート制限は導入しない。単一利用者・ローカル IPC・ホットプラグ間隔 ≥ 2 s で十分と判断（受容リスク）。
- **Final**: 全 33 AC は Requirements Traceability 表および Evidence マトリクスで設計要素に映射済み。Pass A 修正 4 件は final `design.md` で機械的に確認済み。

## Reflected Fixes

| Finding | 対象セクション | 修正概要 | Pass |
| ------- | -------------- | -------- | ---- |
| QA-1 | D-DeviceSelectionService / Responsibilities | 選択変更の直列化・再開中キュー・最新選択のみ再開 | QA |
| QA-2 | D-DeviceSelectionService / Responsibilities | 同一選択の no-op（不要な再開スキップ） | QA |
| QA-3 | D-DeviceSelectionService / Responsibilities | `error` フェーズからの `restart_with_selection` 回復経路 | QA |
| QA-4 | Testing Strategy / Unit Tests | idempotent・直列化・error 回復の単体テスト 3 件追加 | QA |
| SEC-1 | Security Considerations | Trust Boundaries + Controls（PII 分類・入力検証・ポーリング制限） | Sec |

## Specialist Summaries

### QA

- **Summary**: 異常系 AC（4.x）・空一覧（1.5）・ホットプラグ（1.4）・サイレントフォールバック禁止（3.4, 4.2）は Error Handling・契約・フローでカバー。派生エッジとして再開中の競合・idempotency・error 回復を追加設計。
- **主要 Decisions**: 選択変更直列化と no-op。`error` からの再選択で回復。

### Arch

- **Summary**: ヘキサゴナル拡張は steering 準拠。`DeviceSelectionStore` が選択状態の単一所有者。PCM / ミキシングは audio-capture 境界内。Persistent References の modify 契約・`boundaries.md` セクション・ADR-0009 が存在し設計と一致（Contract sync: OK）。
- **主要 Decisions**: 拡張シナリオ（永続化追加・cpal 更新）は既存コンポーネント境界内で吸収可能。反パターンスキャン: 該当なし。

### Sec

- **Summary**: 新表面は Tauri IPC（一覧・選択）とローカルイベントのみ。外部送信・AuthN/AuthZ なし。PII（デバイス名）はログ段階分離。Threat table（Evidence）で全表面に mitigation または受容リスクを記録。
- **主要 Decisions**: invoke レート制限は不導入（単一利用者ローカル）。Trust boundary を設計に明示（SEC-1 反映）。

## Gap-Domain Audit

| # | ドメイン | 結果 | 根拠 |
| --- | --- | --- | --- |
| 1 | Requirements traceability | pass | 全 33 AC → 設計 Traceability 表 + Evidence マトリクス。未映射なし |
| 2 | Non-functional (non-security) | pass | 再開 < 2 s（5.3）、ポーリング UI 可視時のみ ≥ 2 s（5.1）、Operational Readiness 記載 |
| 3 | Observability | pass | ログ段階（ID/名前）、metrics、correlation_id。失敗モード（列挙失敗・再開）をカバー |
| 4 | Operability | pass | Deployment & Rollout（契約追加のみ・ロールバックはバイナリ差替）、Migration N/A |
| 5 | Testability | pass | Unit / Integration / E2E / Performance。境界は trait + injectable IPC でテスト可能 |
| 6 | Compatibility | pass | PCM 非変更（reference）。新 command 追加のみ。下流 whisper-transcribe 影響なし |
| 7 | Scope fitness | pass | complexity_tier L、設計 ~430 行は 3 OS 面・再キャプチャ・UI の妥当量。YAGNI 遵守（永続化・仮想デバイス除外） |
| 8 | Internal & external consistency | pass | 契約・ADR・boundaries・steering と矛盾なし。Contract sync: OK |

## 承認ゲートサマリ

### 検証済み観点

- Pass A QA / Arch / Sec 完了。Reflected Fixes 5 行すべて final `design.md` に存在（反映検証 pass）
- ギャップドメイン 1–8: すべて pass（上表）
- Reviewed Scope: 契約 3 件・ADR 2 件 Read。Contract sync: OK
- 上流要求ゲート: `requirements-review.md` VERDICT: GO、Phase Gate STATUS: VERIFIED
- `spec.json` `approvals.design.generated === true`、`approved === false`（人間承認前）

### 自己修復した事項

- Pass B による追加修正: なし（Pass A の QA/Sec 修正のみ）

### 受容が必要な残リスク

1. **再開時間 2 s 目標の実機ばらつき（要件 5 AC3）** — 設計で目標化済み。却下時は受入テスト基準の調整が必要。
2. **macOS スピーカー選択の UX 期待ギャップ（ADR-0009）** — OS 既定出力一致が必要。UI ヘルプテキストで緩和。実機文言は E2E で検証。
3. **invoke レート制限なし** — 単一利用者ローカル前提。悪用シナリオは低と判断。

### 人間判断が必要な未決事項

- 0 件（上記残リスク 3 件は受容判断のみ）

## Evidence

### 参照ファイル

- `docs/specs/audio-device-selection/design.md`（Pass A 修正後）
- `docs/specs/audio-device-selection/requirements.md`
- `docs/specs/audio-device-selection/research.md`
- `docs/specs/audio-device-selection/spec.json`
- `docs/specs/audio-device-selection/reviews/requirements-review.md`
- `docs/steering/tech.md`, `structure.md`
- Persistent References: `docs/contracts/audio-device-selection.md`, `audio-capture-status.md`, `audio-capture-pcm.md`
- `docs/architecture/boundaries.md`, `docs/architecture/adr/ADR-0001-platform-audio-capture.md`, `ADR-0009-macos-speaker-selection-strategy.md`

### Unwanted Behavior AC → Design Coverage

| AC | 設計カバレッジ |
| --- | --- |
| 4.1 マイク不能 | `SELECTED_MIC_UNAVAILABLE`、`CaptureOrchestrator` error 停止 |
| 4.2 スピーカー不能（フォールバック禁止） | `SELECTED_SYSTEM_AUDIO_UNAVAILABLE`、マイク単独継続禁止（Unit Test 5） |
| 4.3 切断・権限失効 | `DEVICE_DISCONNECTED`、安全停止 |
| 4.4 行動可能通知 | `action_ja`（契約 + `CaptureEventEmitter`） |
| 4.5 再選択可能 | UI 維持、`error` フェーズ回復経路（QA-3 反映） |

### Derived Edge Cases → Design Coverage

| エッジケース | 結果 |
| --- | --- |
| 空一覧（1.5） | `DeviceSelectorPanel` empty state — pass |
| 一覧外 ID 指定 | `INVALID_DEVICE` — pass |
| 再開中の連続選択 | 直列化・最新のみ再開（QA-1 反映） — pass |
| 同一選択再送信 | no-op（QA-2 反映） — pass |
| ホットプラグで選択デバイス消失 | `devices-changed` + キャプチャ中は `DEVICE_DISCONNECTED` — pass |
| macOS 非既定スピーカー | `MACOS_OUTPUT_NOT_DEFAULT` preflight — pass |
| 列挙失敗 | `INTERNAL` invoke、`WARN` ログ — pass |
| `error` からの回復 | `restart_with_selection`（QA-3 反映） — pass |

### Threat Table (STRIDE)

| # | Surface | Threat (STRIDE) | Impact | Mitigation in design / Accepted risk |
| --- | --- | --- | --- | --- |
| 1 | `list_audio_devices` | I: デバイス名漏洩 | 環境情報の露出 | ローカル IPC のみ（7.3）。ログは ID 優先・名前 DEBUG 限定（Observability, Security Controls） |
| 2 | `set_device_selection` | T: 不正ペイロード | 意図しないデバイス切替 | `DeviceSelection` 形状検証 + 一覧外 ID は `INVALID_DEVICE`（Security Controls, 契約） |
| 3 | `set_device_selection` | D: 連続 invoke | CPU / 再開ストーム | 直列化 + idempotent no-op（D-DeviceSelectionService）。レート制限なしは受容（Decisions SEC-2） |
| 4 | `devices-changed` event | I: 一覧の傍受 | デバイス構成の露出 | 同一プロセス WebView のみ。外部送信禁止（7.2–3） |
| 5 | ログ / metrics | I: PII in logs | デバイス名の記録 | `INFO` は ID のみ。名前 DEBUG 限定（Observability） |
| 6 | OS デバイス API | D: ポーリング濫用 | 会議アプリへの負荷 | UI 可視時のみ、間隔 ≥ 2 s（5.1, Operational Readiness） |
| 7 | 音声 PCM 経路 | I: 外部送信 | 会議音声漏洩 | 変更なし — 既存 audio-capture ローカル境界（7.2） |
| 8 | AuthN/AuthZ | E: 権限昇格 | N/A | 単一利用者ローカル。認証 N/A（7.4） — 受容 |

### Requirements → Design Traceability Matrix（全 AC）

| AC | 設計要素 |
| --- | --- |
| 1.1 | D-AudioDeviceEnumerator, D-DeviceSelectionCommands |
| 1.2 | D-AudioDeviceEnumerator |
| 1.3 | D-AudioDeviceEnumerator (`Device::name`) |
| 1.4 | D-DeviceSelectionService (`set_ui_visible`, devices-changed) |
| 1.5 | D-DeviceSelectorPanel empty state |
| 2.1 | D-DeviceSelectorPanel, App chrome |
| 2.2 | D-DeviceSelectionStore, `set_device_selection` |
| 2.3 | D-DeviceSelectionStore, `set_device_selection` |
| 2.4 | D-DeviceSelectorPanel, D-useAudioDevices |
| 2.5 | D-DeviceSelectionStore (`None` = default) |
| 2.6 | D-TauriLifecycleHook, 起動 `start` |
| 3.1 | D-CaptureOrchestrator `restart_with_selection` |
| 3.2 | D-ChunkEmitter, audio-capture-pcm（reference） |
| 3.3 | D-CaptureOrchestrator `restart_with_selection` |
| 3.4 | D-CaptureOrchestrator, D-DeviceSelectionService（サイレント切替禁止） |
| 3.5 | ADR-0001, ADR-0009 |
| 4.1 | `SELECTED_MIC_UNAVAILABLE` |
| 4.2 | `SELECTED_SYSTEM_AUDIO_UNAVAILABLE` |
| 4.3 | `DEVICE_DISCONNECTED` |
| 4.4 | D-CaptureEventEmitter `action_ja` |
| 4.5 | D-DeviceSelectorPanel, error 回復経路 |
| 5.1 | D-AudioDeviceEnumerator, UI 可視時のみポーリング |
| 5.2 | 既存 audio-capture パイプライン継承 |
| 5.3 | D-CaptureOrchestrator < 2 s 目標 |
| 6.1 | D-MacScreenCaptureKitAdapter, ADR-0009 preflight |
| 6.2 | D-WindowsLoopbackAdapter per-device loopback |
| 6.3 | D-TauriLifecycleHook cfg ガード |
| 7.1 | D-CaptureOrchestrator preflight |
| 7.2 | D-PcmChunkBus（既存） |
| 7.3 | D-DeviceSelectionCommands ローカル IPC |
| 7.4 | —（N/A、設計で明記） |
| 7.5 | D-CaptureEventEmitter permission codes |

### チェック項目結果（抜粋）

| チェック | 結果 |
| --- | --- |
| QA: Unwanted Behavior AC 映射 | pass |
| QA: 派生エッジケース | pass（QA-1–4 反映後） |
| QA: Testing Strategy 異常系 | pass |
| Arch: レイヤ・依存方向 | pass |
| Arch: 反パターンスキャン | pass（該当なし） |
| Arch: Contract sync | OK |
| Arch: 拡張シナリオ 2 件 | pass |
| Sec: Threat table | pass |
| Sec: PII / Observability | pass |
| Sec: AuthN/AuthZ | N/A |
| Final: Reflected Fixes 検証 | pass（5/5） |
| Final: 専門パス間矛盾 | pass |
| Phase Gate #1 design.md | pass |
| Phase Gate #2 generated === true | pass |
| Phase Gate #5 approved === false | pass |

## Phase Gate
- STATUS: VERIFIED
- CHECKS:
  1. `docs/specs/audio-device-selection/design.md` 存在・設計内容あり — **pass**
  2. `spec.json` → `approvals.design.generated === true` — **pass** (`true`)
  3. `reviews/design-review.md` → `VERDICT: GO` — **pass**（本ファイル）
  4. Phase Gate `STATUS: VERIFIED` — **pass**（本セクション）
  5. `approvals.design.approved === false` — **pass** (`false`, 人間承認前)
