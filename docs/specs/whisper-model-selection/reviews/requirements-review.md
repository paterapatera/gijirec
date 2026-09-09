## Verdict
- VERDICT: GO

## Summary

`whisper-model-selection` の要求フェーズを PO / QA / Sec の統合レビューで検証した。brief の Scope In/Out、既存 whisper-transcribe 拡張としての brownfield 前提、3 バリアント選択・取得・永続化・状態表示・後方互換が要件 1–5 で網羅されている。QA 指摘（ローカル既存モデル時の取得スキップ、初回起動時の既定バリアント）を requirements.md に反映済み。セキュリティ面はローカル設定のみで認証不要、機微データ非含有を AC で明示。Phase Gate は VERIFIED。

## Findings

| ID | Severity | Domain | Finding | Disposition |
|----|----------|--------|---------|-------------|
| PO-1 | Minor | Scope | 初回起動時の既定バリアントが未記載 | Fixed — 要件 4 AC 3 で FP16 既定を明示 |
| QA-1 | Major | Testability | ローカルにモデルが既存の場合の observable 挙動が不足 | Fixed — 要件 2 AC 2 を追加 |
| QA-2 | Minor | Testability | 要件 5 AC 2「既存利用者と同等」はやや抽象 | Accepted — 後方互換の回帰テストで検証可能と判断 |
| Sec-1 | Minor | Trust boundary | モデル取得の HTTPS / SHA 検証は既存 whisper-transcribe パターンに委譲 | Deferred to design — 設計で既存 ModelDownloader 契約を踏襲 |

## Decisions

- **初回既定バリアント**: 保存済み選択がない場合は現行運用と同等の **FP16** を既定とする（既存 ADR-0011 / brownfield 後方互換）。
- **ホットスワップ**: v1 では転写中の即時切替を要求せず、次回推論から反映（brief Scope Out と一致）。
- **セキュリティ**: デスクトップローカルアプリのため AuthN/AuthZ は N/A。設定永続化に転写・PCM・認証情報を含めない（要件 4 AC 5）。モデル取得の trust boundary は設計フェーズで既存インフラ契約を参照。
- **隣接 spec**: transcribe-volume-normalize は独立。whisper-transcribe-status / blocks の契約維持を要件 5 で要求。

## Reflected Fixes

| Finding | 対象セクション | 修正概要 | Pass |
| ------- | -------------- | -------- | ---- |
| PO-1 | 要件 4 受け入れ条件 | 初回起動時 FP16 既定 AC を追加 | PO |
| QA-1 | 要件 2 受け入れ条件 | ローカル既存モデル時は追加取得なしで読み込み AC を追加 | QA |
| Final-1 | 要件 4 AC 3–4 | AC 番号を再採番（PO/QA 追加分に整合） | Final |

## Specialist Summaries

### PO

- brief の Trigger / Problem / Desired Outcome / Scope In/Out を要件 1–5 にトレース可能。
- スコープ境界で transcribe-volume-normalize との責務分離を明示。
- 3 バリアント限定、オフライン運用、次回推論からの切替を AC 化済み。

### QA

- 全 AC が EARS 形式かつ observable trigger → outcome を持つ。
- 異常系: 取得失敗（2.6）、永続化失敗（4.4）、回復不能エラー（3.4）をカバー。
- 境界: 初回既定（4.3）、ローカル既存（2.2）、転写中切替 defer（2.4）を追加。

### Sec

- AuthN/AuthZ: N/A（単一利用者ローカルデスクトップ）。
- 機微データ: 設定に転写・PCM・認証情報を含めない AC あり。
- モデルダウンロードの改ざん対策は既存 feature の設計契約に委譲（deferred）。

## Gap-Domain Audit

| # | Domain | Result |
|---|--------|--------|
| 1 | Brief traceability | Pass — 全 Scope In/Out を要件またはスコープ境界でカバー（Evidence 参照） |
| 2 | Cross-spec consistency | Pass — roadmap 依存 none。whisper-transcribe / editor-settings パターンと矛盾なし |
| 3 | NFR completeness | Pass — オフライン運用（2.5）、後方互換（要件 5）を user-observable AC で記載 |
| 4 | Operability | Pass — エラー時 action_ja 相当の日本語メッセージ要求（2.6, 3.4, 4.4） |
| 5 | Compliance | N/A — steering に追加規制要件なし |
| 6 | Template conformance | Pass — はじめに、スコープ境界、数値要件 ID、目的、受け入れ条件、ja + EARS 英語キーワード |
| 7 | Scope fitness | Pass — brief 外の gold-plating なし、brief 項目の脱落なし |
| 8 | Terminology | Pass — Q5_0 / Q8_0 / FP16、kotoba-whisper-v2.2 を一貫使用 |

## 承認ゲートサマリ

### 検証済み観点

- PO: 意味的一貫性・スコープ境界・brief 整合 — Pass
- QA: AC 検証可能性・異常系・境界条件 — Pass（2 件修正反映）
- Sec: 機微データ非含有・AuthN N/A — Pass（取得 trust boundary は design 委譲）
- Gap domains 1–8 — Pass / N/A（上表）
- Reflection verification — 全 Reflected Fixes を requirements.md で確認済み

### 自己修復した事項

- 要件 4 AC 番号の再採番（Final gate）

### 受容が必要な残リスク

- **モデル取得 trust boundary（HTTPS / SHA-256）**: 要件では既存 whisper-transcribe パターン踏襲を前提とし、詳細は設計で ModelDownloader 契約を参照する。設計承認時に sec 再確認を推奨。
- **要件 5 AC 2 の回帰範囲**: 「既存利用者と同等」は E2E 回帰テストで具体化する。設計・タスクでテスト観点を明記すること。

### 人間判断が必要な未決事項

- なし（0 件）

## Evidence

### Brief → Requirements Traceability Matrix

| Brief 項目 | 要件 / AC |
|-----------|-----------|
| Trigger: 3 段階切り替え要望 | 要件 1 |
| Problem: モデル選べない | はじめに、要件 1 |
| Desired: Q5_0/Q8_0/FP16 選択・反映・未取得取得・永続化 | 要件 1–4 |
| Scope In: 選択 UI/設定 | 要件 1 AC 1 |
| Scope In: 取得・ロード・転写適用 | 要件 2 |
| Scope In: 状態表示 | 要件 3 |
| Scope In: 永続化 | 要件 4 |
| Scope Out: 他ファミリ / カスタム / クラウド / 自動推奨 / 他量子化 / ホットスワップ | スコープ境界、要件 1 AC 3–4、要件 2 AC 4、要件 5 AC 3 |
| Constraint: オフライン運用 | 要件 2 AC 5 |
| Constraint: IPC 後方互換 | 要件 5 |
| Approach: 設定でバリアント選択 | 要件 1, 4 |

### Phase Gate Inline Checks

| # | Check | Result |
|---|-------|--------|
| 1 | requirements.md exists with requirement/AC content | OK |
| 2 | spec.json approvals.requirements.generated === true | OK |
| 3 | VERDICT: GO | OK |
| 4 | Phase Gate STATUS: VERIFIED | OK |
| 5 | approvals.requirements.approved === false | OK |

## Phase Gate
- STATUS: VERIFIED
- CHECKS: requirements.md 存在・AC 記載 / spec.json generated=true / VERDICT GO / 人間未承認 — すべて合格
