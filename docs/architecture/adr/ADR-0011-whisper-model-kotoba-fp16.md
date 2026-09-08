# ADR-0011: デフォルト Whisper モデルを kotoba-whisper-v2.2（GGML FP16）に昇格

- **Status**: Accepted
- **Date**: 2026-09-08
- **Feature**: whisper-transcribe
- **Owners / Domains**: whisper-transcribe
- **Supersedes**: ADR-0010 の量子化ランク（Q8_0 → FP16）判断のみ（kotoba-whisper 採用・配布元・保存先規約は ADR-0004 / ADR-0008 を維持）

## Context

ADR-0010 では転写精度向上のため kotoba-whisper-v2.2 の **Q8_0**（約 818 MB）をデフォルトとした。利用者からさらに 1 ランク上の品質を求める要望があり、kenrouse 配布の **FP16 原版**（`kotoba-whisper-v2.2-ggml.bin`、約 1.52 GB）への昇格を検討する。

FP16 は kenrouse 配布で「最高品質（best quality）」と位置づけられ、whisper.cpp 互換 GGML のまま ADR-0003 のストリーミング API を維持できる。

## Decision

デフォルトモデルを **Q8_0 から FP16 原版に昇格**する。

| 項目 | 値 |
|------|-----|
| ファイル名 | `kotoba-whisper-v2.2-ggml.bin` |
| 配布元 | HuggingFace `kenrouse/kotoba-whisper-v2.2-ggml` |
| 量子化 | FP16（非量子化・最高精度・推論コスト最大） |
| 保存先 | `{app_data_dir}/models/`（`ModelStore::MODEL_FILENAME`、ADR-0008） |
| 取得 URL / SHA-256 | `src-tauri/src/compose.rs` の `DEFAULT_WHISPER_MODEL_*` 定数 |

モデル変更時は URL・SHA-256・`MODEL_FILENAME` を同時更新し、破損検出（`ModelCorrupt`）で再取得を促す。旧 Q8_0 ファイルは SHA 不一致または手動削除後に FP16 を再取得する。

## Consequences

- Positive: 日本語転写精度・句読点品質が Q8_0 より向上する余地がある（特に雑音・音楽混在環境）
- Positive: kotoba-whisper ファミリ・whisper-cpp-plus ストリーミング方式は変更しない
- Negative / trade-offs: モデルサイズ（約 +700 MB）・推論 latency・常駐メモリが大幅増加し、要件 7（5 秒以内表示・CPU 予算）の再検証が必須
- Negative / trade-offs: 初回ダウンロード時間・ディスク使用量が増大する

## Alternatives considered

1. **Q8_0 維持（ADR-0010）** — リソースと latency のバランスには有利だが、さらなる精度向上要望に応えられない
2. **F32（非量子化フル精度）** — 約 3 GB と過大。リアルタイム会議用途では採用しない
3. **ユーザー向けモデル選択 UI** — スコープ外。将来必要なら別 spec / ADR で検討

## Notes

- ライブラリ（whisper-cpp-plus）・VAD・ストリーミング方式は ADR-0003 のまま
- 実機性能（3 s 発話 → 5 s 以内ブロック、CPU / メモリ）は `docs/manual/whisper-transcribe/performance-results.md` に記録する
- 判断を覆す場合は新 ADR を作成し、本 ADR の Status を `Superseded by ADR-XXXX` に変更する
