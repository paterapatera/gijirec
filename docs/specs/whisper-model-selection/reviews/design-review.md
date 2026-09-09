## Verdict
- VERDICT: GO

## Summary

`whisper-model-selection` の設計を QA / Arch / Sec の統合レビューで検証した。3 バリアントカタログ + `ModelStore` / `ModelOrchestrator` 拡張 + `whisper-transcribe-settings` 新契約 + ADR-0013 で要件 1–5 をトレース可能。既存 `whisper-transcribe-status` は reference のみで後方互換を維持。Sec deferred（HTTPS/SHA）を設計でクローズ。Phase Gate は VERIFIED。

## Reviewed Scope
- Reviewed contract paths: `docs/contracts/whisper-transcribe-settings.md` (modify), `docs/contracts/whisper-transcribe-status.md` (reference)
- ADR paths: `ADR-0013-whisper-model-variant-selection.md`, `ADR-0011-whisper-model-kotoba-fp16.md`, `ADR-0008-model-store-app-data-dir.md`
- Contract sync: OK

## Findings

| ID | Severity | Domain | Finding | Disposition |
|----|----------|--------|---------|-------------|
| QA-1 | Minor | Idempotency | 同一バリアント再選択時の挙動が未記載 | Fixed — ModelOrchestrator に no-op 明記 |
| Arch-1 | Minor | Extension | 将来 4 バリアント追加時は Catalog + contract 更新が必要 | Accepted — Revalidation Triggers に記載済み |
| Sec-1 | Minor | Threat model | requirements-review で design 委譲の DL trust boundary | Fixed — design Security + ModelOrchestrator に HTTPS/SHA 明記 |

## Decisions

- **フェーズイベント非変更**: `whisper-transcribe-status.md` は reference。UI は既存 `useTranscribeStatus` + settings snapshot で要件 3 を満たす
- **転写中切替**: `pending_variant` をバッチサイクル境界でのみ適用（要件 2.4）
- **初回既定 fp16**: 既存 `kotoba-whisper-v2.2-ggml.bin` 互換（要件 4.3, 5.4）
- **Sec accepted risk**: ローカルデスクトップのため rate limit / AuthN は N/A。モデル改ざんは SHA-256 で軽減

## Reflected Fixes

| Finding | 対象セクション | 修正概要 | Pass |
| ------- | -------------- | -------- | ---- |
| QA-1 | Components — ModelOrchestrator | 同一 variant 再設定は no-op | QA |

## Specialist Summaries

### QA

- 異常系: DL 失敗（2.6）、永続化失敗（4.4）、corrupt model（既存 ModelCorrupt）を design でカバー
- 境界: 初回 fp16、ローカル既存スキップ DL、転写中 defer、同一 variant 再選択 no-op
- 並行: `pending_variant` + サイクル境界でレース回避
- Testing Strategy に unit/integration/E2E 観点あり

### Arch

- レイヤ: domain catalog → application orchestrator/settings → infrastructure store → presentation commands/UI
- 既存 asset 再利用: editor-settings 永続化パターン、ModelDownloader/Worker、compose inject
- File Structure Plan: コンポーネントとファイル 1:1 対応、god object なし
- ADR-0013 で境界変更を記録。Contract sync OK
- Extension: 追加バリアントは Catalog + contract 行追加で吸収可能（依存方向違反なし）

### Sec

| # | Surface | Threat (STRIDE) | Impact | Mitigation |
| - | ------- | --------------- | ------ | ---------- |
| 1 | HTTPS model DL | Tampering (T) | 悪意あるモデル注入 | HTTPS-only + SHA-256 verify（Security Considerations） |
| 2 | transcribe-settings.json | Tampering (T) | 不正 variant 設定 | 列挙検証 `INVALID_MODEL_VARIANT`、破損時 fp16 fallback |
| 3 | Settings persistence | Information disclosure (I) | 機微データ漏洩 | 転写・PCM・認証情報を含めない（要件 4.5） |
| 4 | Logs | Information disclosure (I) | 転写内容ログ漏洩 | URL 全文・転写・PCM ログ禁止（Observability） |

- AuthN/AuthZ: N/A
- Supply chain: kenrouse 固定 URL + SHA ピン（ModelVariantCatalog）

## Gap-Domain Audit

| # | Domain | Result |
|---|--------|--------|
| 1 | Requirements traceability | Pass — 要件 1–5 全 ID が Traceability 表に存在 |
| 2 | Contract alignment | Pass — settings 契約と design 一致、status は reference |
| 3 | Architecture boundaries | Pass — boundaries.md セクション追加済み |
| 4 | NFR / operability | Pass — オフライン、rollback、ディスク見積り |
| 5 | Security | Pass — Sec deferred 項目クローズ |
| 6 | Testability | Pass — Testing Strategy が AC 異常系を参照 |
| 7 | Scope fitness | Pass — brief 外の gold-plating なし |
| 8 | Steering alignment | Pass — tech.md / structure.md レイヤパターン一致 |

## 承認ゲートサマリ

### 検証済み観点

- QA: 異常系・境界・並行 — Pass（QA-1 修正反映）
- Arch: レイヤ・契約同期・ADR — Pass
- Sec: 機微データ・DL trust boundary — Pass
- Gap domains 1–8 — Pass
- Reflection verification — QA-1 を design.md で確認済み

### 自己修復した事項

- ModelOrchestrator 同一 variant 再設定 no-op（QA-1）

### 受容が必要な残リスク

- **3 バリアント全保持時のディスク ~2.8 GB**: 利用者選択による段階 DL。設計 Operational Readiness に記載
- **転写中切替の実装レース**: サイクル境界フックの統合テストで検証（Testing Strategy に記載）

### 人間判断が必要な未決事項

- なし（0 件）

## Evidence

### Unwanted Behavior AC Coverage

| AC | Design coverage |
|----|-----------------|
| 2.6 DL 失敗 | ModelOrchestrator → `whisper-transcribe://error` |
| 3.4 回復不能エラー | 同上 + error phase |
| 4.4 永続化失敗 | fp16 fallback + 日本語通知 |

### Phase Gate Inline Checks

| # | Check | Result |
|---|-------|--------|
| 1 | design.md exists | OK |
| 2 | approvals.design.generated === true | OK |
| 3 | VERDICT: GO | OK |
| 4 | Phase Gate STATUS: VERIFIED | OK |
| 5 | approvals.design.approved === false | OK |

## Phase Gate
- STATUS: VERIFIED
- CHECKS: design.md 存在 / generated=true / VERDICT GO / 人間未承認 — すべて合格
