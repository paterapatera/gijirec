## Verdict
- VERDICT: GO

## Summary

`capture-audio-controls` の要件定義は brief・steering・完了済み upstream spec（`audio-device-selection`、`transcribe-volume-normalize`）と整合しており、5 要件・24 AC でコア機能をカバーしている。Pass A でスコープ明確化（OS ミュートとの境界）、テスト可能性（dBFS 更新頻度）、初期ゲイン既定、信頼境界の 4 点を `requirements.md` に反映済み。Pass B の反映検証・8 ドメイン監査・フェーズゲートはすべて合格。

## Findings

| ID | 重大度 | Pass | 内容 | 処置 |
| --- | --- | --- | --- | --- |
| PO-1 | Major | PO | マイク OFF が OS レベルミュートと誤読されるリスク | スコープ境界「対象外」に ingest ミックス除外のみと明記（反映済み） |
| PO-2 | Major | PO | 手動ゲイン導入時の既存固定ゲイン（×1.25）との連続性が未定義 | 要件 3 AC 6 を追加（反映済み） |
| QA-1 | Major | QA | 要件 2 AC 2 の「near-real-time cadence」が測定不能 | 「at least once per second」に具体化（反映済み） |
| Sec-1 | Major | Sec | 認証・IPC 信頼境界・メタデータのみ配信の要件レベル記述が不足 | スコープ境界に「信頼境界・データ取扱い」を追加（反映済み） |
| QA-2 | Minor | QA | 要件 3 AC 4 のクリッピング防止は設計で具体化が必要 | 設計フェーズへ委譲（残リスクとして記録） |
| Sec-2 | Minor | Sec | IPC コマンド形状の詳細 threat model | 設計フェーズへ委譲（既存 device selection と同一信頼モデル） |

## Decisions

- **PO**: マイク OFF は gijirec 内の転写 ingest ミックスからマイクを除外する操作であり、OS や他アプリのマイク入力を遮断しない。
- **PO**: セッション内でユーザーが手動ゲインを変更していない場合、既存 `transcribe-volume-normalize` の ×1.25 + ソフトリミット 0.95 と等価な loudness を維持する（要件 3 AC 6）。
- **QA**: brief の「近リアルタイム」は要件レベルでは「1 秒に 1 回以上の dBFS 更新」として観測可能化する。上限頻度は設計で決定。
- **Sec**: AuthN/AuthZ はローカル単一ユーザーデスクトップアプリのため N/A。追加の認証要件は不要。
- **Sec**: dBFS メーターはレベル表示用メタデータのみフロントへ配信し、生 PCM は転送しない（要件 2 AC 5 で既存、スコープ境界で補強）。
- **Sec（deferred）**: マイクトグル・ゲイン調整の IPC コマンド詳細と入力検証は設計 threat model で扱う。既存 `audio-device-selection` と同一信頼境界を前提とする。
- **Final**: `transcribe-volume-normalize` 固定ゲインの「置き換えまたは上書き」詳細は brief の Out（先取りしない）に従い設計フェーズで決定。要件では基準点と初期値のみ規定。

## Reflected Fixes
| Finding | 対象セクション | 修正概要 | Pass |
| ------- | -------------- | -------- | ---- |
| PO-1 | スコープ境界・対象外 | OS レベルマイクミュートは対象外、ingest ミックス除外のみと明記 | PO |
| PO-2 | 要件 3・受け入れ条件 | AC 6 追加：未調整時は `transcribe-volume-normalize` 固定ゲインと等価 | PO |
| QA-1 | 要件 2・受け入れ条件 AC 2 | 「near-real-time cadence」→「at least once per second」 | QA |
| Sec-1 | スコープ境界 | 「信頼境界・データ取扱い」節を追加（ローカル運用・メタデータのみ・IPC 信頼境界） | Sec |

## Specialist Summaries

### PO
5 要件が brief の In/Out と 1:1 で対応し、隣接 spec への期待もスコープ境界に明示済み。主要判断はマイク OFF の意味域（ingest ミックス限定）と、手動ゲイン導入時の既存 loudness 連続性（AC 6）。矛盾する AC はなし。

### QA
全 AC に観測可能なトリガーと結果があり、異常系（音声源なし、非キャプチャ時 UI 無効、エラー表示、リソース圧迫時の graceful degradation）もカバー。dBFS 更新頻度を 1 Hz 下限で具体化しテスト可能にした。ゲイン上下限の具体値は設計委譲（残リスク）。

### Sec
機密データの新規収集なし。生 PCM 非配信（要件 2 AC 5）が主要制御。認証 N/A、信頼境界をスコープ境界に追記。IPC 詳細は設計 defer。

## Gap-Domain Audit

| # | ドメイン | 結果 | 備考 |
| --- | --- | --- | --- |
| 1 | Brief traceability | pass | 全 Problem/Scope/Constraints が要件またはスコープ境界でカバー（Evidence にマトリクス） |
| 2 | Cross-spec consistency | pass | `audio-device-selection`・`transcribe-volume-normalize`・roadmap と矛盾なし |
| 3 | NFR completeness | pass | 要件 5 で性能・安定性を規定。brief の性能制約を反映 |
| 4 | Operability expectations | N/A | 新規運用監視・データ保持要件なし。既存 capture エラー／ログパターン継続 |
| 5 | Compliance | N/A | オフライン単一ユーザー。追加規制要件なし |
| 6 | Template conformance | pass | はじめに・スコープ境界・要件 N + 目的 + 受け入れ条件・数値 ID・EARS 英語キーワード |
| 7 | Scope fitness | pass | brief 外の gold-plating なし。brief In 項目の欠落なし |
| 8 | Terminology & consistency | pass | dBFS・スピーカー／システム音声・ingest 前処理の用語が一貫 |

## 承認ゲートサマリ

### 検証済み観点
- Pass A PO/QA/Sec 完了、全 Reflected Fixes を `requirements.md` で機械確認済み
- 反映検証：Pass 間の矛盾なし（PO 判断と QA/Sec 修正は整合）
- ギャップドメイン 1–8：pass 6 件、N/A 2 件（Operability、Compliance）
- フェーズゲート 4 チェックすべて合格

### 自己修復した事項
- Pass B による `requirements.md` 追加修正なし（Pass A で完結）

### 受容が必要な残リスク
- **ゲイン上下限の具体値**（要件 3 AC 4）：設計でクリッピング防止の実装詳細を決定。却下時はテスト不能な NFR が残る。
- **IPC threat model**（Sec deferred）：既存 device selection 同等の信頼モデルで設計。却下時はセキュリティレビュー再開が必要。
- **`transcribe-volume-normalize` 置換方針**：設計で固定ゲインとの統合方式を決定。却下時は ingest パイプライン仕様の再定義が必要。

### 人間判断が必要な未決事項
- 0 件（上記残リスクは設計フェーズでの通常判断として文書化済み）

## Evidence

参照ファイル:
- `docs/specs/capture-audio-controls/spec.json` — pass（`approvals.requirements.generated: true`、`language: ja`）
- `docs/specs/capture-audio-controls/brief.md` — pass
- `docs/specs/capture-audio-controls/requirements.md` — pass（Pass A 修正後 5 要件・24 AC）
- `docs/steering/product.md` — pass（dual-capture、ingest ゲイン、offline 前提）
- `docs/steering/tech.md` — pass（`PcmIngestConsumer`、RMS 可観測性）
- `docs/steering/structure.md` — pass（`DeviceSelectorPanel`、IPC パターン）
- `docs/steering/roadmap.md` — pass（`capture-audio-controls` 計画、deps: none）
- `docs/settings/templates/specs/requirements.md` — pass（構造一致）

### Brief → Requirements トレーサビリティマトリクス

| Brief 項目 | カバー先 |
| --- | --- |
| Trigger: スピーカーのみ転写したい | 要件 1 |
| Trigger: dB 確認＋手動ゲイン調整 | 要件 2, 3 |
| Problem: マイクを止められない | 要件 1（目的・AC 1–4） |
| Problem: 固定ゲインで調整不可 | 要件 3 |
| Desired: マイク OFF → スピーカーのみ | 要件 1 AC 2 |
| Desired: dB（dBFS）表示 | 要件 2 |
| Desired: −18〜−17 dBFS 手動調整 | 要件 3 AC 3 |
| In: マイク ON/OFF | 要件 1 |
| In: dB 表示（近リアルタイム） | 要件 2 AC 2 |
| In: 手動ゲイン UI | 要件 3 |
| In: デバイス選択パネル整合 | 要件 4 |
| Out: OS ミキサー代替 | スコープ境界・対象外 |
| Out: 自動 AGC | スコープ境界・対象外 |
| Out: 仮想オーディオ | スコープ境界・対象外、要件 4 AC 4 |
| Out: エクスポート・話者分離 | スコープ境界・対象外 |
| Constraints: 仮想デバイス不要 | 要件 4 AC 4 |
| Constraints: 性能を大きく損なわない | 要件 5 |

### 反映検証（Pass B Step 1）
- PO-1 → `requirements.md` L10 対象外に OS ミュート除外明記 — **verified**
- PO-2 → `requirements.md` L52 要件 3 AC 6 — **verified**
- QA-1 → `requirements.md` L36 要件 2 AC 2「at least once per second」— **verified**
- Sec-1 → `requirements.md` L11 信頼境界・データ取扱い — **verified**

## Phase Gate
- STATUS: VERIFIED
- CHECKS:
  1. `docs/specs/capture-audio-controls/requirements.md` 存在・要件/AC 内容あり — **pass**（5 要件、24 AC）
  2. `spec.json` → `approvals.requirements.generated === true` — **pass**
  3. `reviews/requirements-review.md` → `VERDICT: GO` — **pass**
  4. Phase Gate `STATUS: VERIFIED` — **pass**（本レポート）
