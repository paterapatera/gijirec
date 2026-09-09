# ADR-0013: kotoba-whisper-v2.2 の 3 バリアント（Q5_0 / Q8_0 / FP16）ユーザー選択

- **Status**: Accepted
- **Date**: 2026-09-09
- **Feature**: whisper-model-selection
- **Owners / Domains**: whisper-model-selection
- **Related**: ADR-0011（FP16 既定維持）、ADR-0008（`app_data_dir/models/` 保存先）

## Context

ADR-0011 では精度優先で FP16 を単一デフォルトとし、「ユーザー向けモデル選択 UI」はスコープ外とした。利用者から用途・マシン性能に応じた量子化切替の要望があり、同一 kenrouse 配布内の Q5_0 / Q8_0 / FP16 の 3 段階選択を別 spec で提供する。

既存実装は `ModelStore` が単一 `kotoba-whisper-v2.2-ggml.bin` を想定。複数バリアント共存と永続化が必要。

## Decision

1. **選択可能バリアント**を `q5_0` / `q8_0` / `fp16` に限定（他ファミリ・量子化は非対象）。
2. **ローカル保存**は `{app_data_dir}/models/` にバリアント別ファイルを共存（各 filename は `whisper-transcribe-settings.md` 正本）。
3. **永続化**は `transcribe-settings.json` の `model_variant`。初回・欠落時の論理既定は **`fp16`**（既存利用者の `kotoba-whisper-v2.2-ggml.bin` を追加取得なしで利用、要件 5.4）。
4. **切替タイミング**: 転写実行中は現サイクルを完了し、**次バッチサイクル**から新バリアント（v1 ホットスワップ非対応）。
5. **公開契約**: 新規 `whisper-transcribe-settings.md`。`whisper-transcribe-status.md` のフェーズ・進捗イベント形状は変更しない。
6. **SHA-256 検証**は既存 `ModelDownloader` / `ModelStore` パターンをバリアント別に適用（HTTPS only、要件 Sec deferred 項目の設計クローズ）。

## Consequences

- Positive: 精度・速度・メモリのトレードオフを利用者が状況に応じて選択可能
- Positive: 既存フェーズ UI・イベント購読を維持し後方互換を確保
- Negative / trade-offs: 3 バリアントすべて保持時はディスク ~2.8 GB。初回 DL 時間・推論コストはバリアント依存
- Negative / trade-offs: `ModelOrchestrator` / `ModelStore` の状態管理が増加

## Alternatives considered

1. **FP16 単一維持（ADR-0011）** — 要望に応えられない
2. **バリアント自動推奨** — v1 スコープ外
3. **単一ファイル上書き DL** — オフラインで他バリアントを保持できず、切替コスト大。不採用

## Notes

- ADR-0011 は「単一デフォルト FP16」判断として **Accepted 維持**。本 ADR はユーザー選択の追加
- 実装定数（URL / SHA）の正本は Rust `ModelVariantCatalog`、契約は `whisper-transcribe-settings.md` と同期
