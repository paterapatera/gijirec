# ADR-0004: デフォルト Whisper モデルを kotoba-whisper-v2.2（GGML Q5_0）に採用

- **Status**: Superseded by ADR-0010
- **Date**: 2026-09-06
- **Feature**: whisper-transcribe
- **Owners / Domains**: whisper-transcribe
- **Supersedes**: ADR-0003 の「デフォルトモデル `ggml-small-q5_0.bin`」判断のみ（ライブラリ選択は ADR-0003 を維持）

## Context

ADR-0003 では whisper-cpp-plus 採用時のデフォルトモデルを OpenAI 公式系の `ggml-small-q5_0.bin`（多言語・量子化）とした。gijirec の主用途は **日本語 Web 会議のリアルタイム議事録**であり、利用者体験では句読点の自然さ・読みやすさが重要になる。

whisper.cpp で動かす場合、kenrouse 氏などが公開している **有志版 GGML モデル**は量子化（軽量化）が洗練されており、日本語の句読点・読点挿入の品質が公式 small 系より実用的なことが多い。会議アプリと並行実行するため、モデルサイズと推論コストのバランスも引き続き重要（要件 7）。

**スコープ外の整理**: whisper.cpp エコシステムには話者分離（diarization）向けのモデル・手法も存在するが、製品要件では話者分離は対象外（要件 2.4）。本 ADR は **日本語転写品質と軽量化**のためのモデル選択であり、話者ラベル付与は行わない。

## Decision

デフォルトモデルを **kotoba-whisper-v2.2** の GGML 量子化版に固定する。

| 項目 | 値 |
|------|-----|
| ファイル名 | `kotoba-whisper-v2.2-ggml-q5_0.bin` |
| 配布元 | HuggingFace `kenrouse/kotoba-whisper-v2.2-ggml` |
| 量子化 | Q5_0（軽量・会議並行利用向け） |
| 保存先 | `{app_data_dir}/models/`（`ModelStore::MODEL_FILENAME`） |
| 取得 URL / SHA-256 | `src-tauri/src/compose.rs` の `DEFAULT_WHISPER_MODEL_*` 定数 |

モデル変更時は URL・SHA-256・`MODEL_FILENAME` を同時更新し、破損検出（`ModelCorrupt`）で再取得を促す。

## Consequences

- Positive: 日本語会議向けの句読点・読みやすさが向上し、議事録のその場確認体験が改善する
- Positive: Q5_0 量子化により、キャプチャと並行した推論でもリソース目標（要件 7）に収まりやすい
- Positive: whisper.cpp 互換 GGML のため ADR-0003 のストリーミング API（`WhisperStreamPcm` + VAD）を維持できる
- Negative / trade-offs: 多言語会議では small 系より言語汎用性が下がる可能性（現 product スコープは日本語会議中心）
- Negative / trade-offs: 有志モデルは upstream 更新・配布 URL 変更の追随が必要（SHA-256 検証で整合性は担保）

## Alternatives considered

1. **`ggml-small-q5_0.bin`（ADR-0003 当初案）** — 多言語だが日本語句読点が弱く、会議議事録 UX に不利
2. **より大型の有志モデル（Q8_0 / 非量子化）** — 精度向上の余地はあるが CPU / メモリ予算（要件 7）に対してコスト増
3. **話者分離付きモデル** — 製品スコープ外。ラベル付き出力は要件 2.4 で禁止

## Notes

- ライブラリ（whisper-cpp-plus）・ストリーミング方式は ADR-0003 のまま
- 判断を覆す場合は新 ADR を作成し、本 ADR の Status を `Superseded by ADR-XXXX` に変更する
