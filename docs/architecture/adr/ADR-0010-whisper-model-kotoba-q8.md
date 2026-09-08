# ADR-0010: デフォルト Whisper モデルを kotoba-whisper-v2.2（GGML Q8_0）に昇格

- **Status**: Superseded by ADR-0011
- **Date**: 2026-09-08
- **Feature**: whisper-transcribe
- **Owners / Domains**: whisper-transcribe
- **Supersedes**: ADR-0004 の量子化ランク（Q5_0 → Q8_0）判断のみ（kotoba-whisper 採用・配布元・保存先規約は ADR-0004 / ADR-0008 を維持）

## Context

ADR-0004 では会議並行利用とリソース目標（要件 7）を優先し、kotoba-whisper-v2.2 の **Q5_0**（約 513 MB）をデフォルトとした。利用者から転写精度を 1 ランク上げる要望があり、同一モデルファミリ内の **Q8_0**（約 818 MB）への昇格を検討する。

Q8_0 は kenrouse 配布および Pomni の量子化表で「品質と速度のバランスが良い」とされるランクであり、whisper.cpp 互換 GGML のまま ADR-0003 のストリーミング API を維持できる。

## Decision

デフォルトモデルの量子化を **Q5_0 から Q8_0 に昇格**する。

| 項目 | 値 |
|------|-----|
| ファイル名 | `kotoba-whisper-v2.2-ggml-q8_0.bin` |
| 配布元 | HuggingFace `kenrouse/kotoba-whisper-v2.2-ggml` |
| 量子化 | Q8_0（精度優先・推論コスト増） |
| 保存先 | `{app_data_dir}/models/`（`ModelStore::MODEL_FILENAME`、ADR-0008） |
| 取得 URL / SHA-256 | `src-tauri/src/compose.rs` の `DEFAULT_WHISPER_MODEL_*` 定数 |

モデル変更時は URL・SHA-256・`MODEL_FILENAME` を同時更新し、破損検出（`ModelCorrupt`）で再取得を促す。旧 Q5_0 ファイルは SHA 不一致または手動削除後に Q8_0 を再取得する。

## Consequences

- Positive: 日本語転写精度・句読点品質が Q5_0 より向上する余地がある（特に雑音環境）
- Positive: kotoba-whisper ファミリ・whisper-cpp-plus ストリーミング方式は変更しない
- Negative / trade-offs: モデルサイズ（約 +300 MB）・推論 latency・常駐メモリが増加し、要件 7（5 秒以内表示・CPU 予算）の再検証が必要
- Negative / trade-offs: 初回ダウンロード時間が長くなる

## Alternatives considered

1. **Q5_0 維持（ADR-0004）** — リソース目標には有利だが、精度向上要望に応えられない
2. **FP16（`kotoba-whisper-v2.2-ggml.bin`、約 1.4 GB）** — さらに精度向上の余地はあるが、リアルタイム会議用途ではコスト過大
3. **ユーザー向けモデル選択 UI** — スコープ外。将来必要なら別 spec / ADR で検討

## Notes

- ライブラリ（whisper-cpp-plus）・VAD・ストリーミング方式は ADR-0003 のまま
- 実機性能（3 s 発話 → 5 s 以内ブロック、CPU / メモリ）は `docs/manual/whisper-transcribe/performance-results.md` に記録する
- 判断を覆す場合は新 ADR を作成し、本 ADR の Status を `Superseded by ADR-XXXX` に変更する
