# ADR-0003: whisper-cpp-plus によるローカルストリーミング STT

- **Status**: Accepted
- **Date**: 2026-09-05
- **Feature**: whisper-transcribe
- **Owners / Domains**: whisper-transcribe

## Context

gijirec は whisper.cpp を Python なしで Rust バイナリ内に組み込み、3〜5 秒相当の音声ウィンドウを逐次文字起こしし、発話終了から 5 秒以内にテキストブロックを下流へ供給する必要がある（要件 2, 3）。無音区間ではブロックを発行しない（要件 3.3）。会議アプリと並行実行されるため CPU 負荷を抑える必要がある（要件 7）。

## Decision

`whisper-cpp-plus`（whisper.cpp v1.8.6-stream-pcm ピン留め）を `gijirec-infrastructure` の STT アダプタとして採用する。

- **ストリーミング API**: `WhisperStreamPcm`（VAD 駆動モード）で PCM 蓄積・セグメント分割・推論を一括処理
- **VAD**: Silero VAD（ライブラリ同梱）で無音区間をスキップ
- **モデル**: デフォルト `ggml-small-q5_0.bin`（多言語・量子化）
- **GPU**: macOS は `metal` feature、Windows は CPU + OpenBLAS（将来 CUDA 検討）
- **スレッドモデル**: 推論は専用ワーカースレッド。`PcmChunkConsumer::on_pcm_chunk` は rtrb push のみ

## Consequences

- Positive: stream-pcm.cpp 移植によりストリーミング・VAD・タイムスタンプが一体提供される。要件 3.2/3.3 に直結
- Positive: `WhisperContext` が `Send + Sync` で Tauri マルチスレッドと整合
- Negative / trade-offs: C++ ビルド依存（CMake）。fork ピン留めの upstream 追従コスト。新規 crate 依存のサプライチェーン監視が必要
- Negative / trade-offs: `whisper-rs` よりエコシステムが小さい（ダウンロード数・メンテナー数）

## Alternatives considered

1. **whisper-rs + 自前ストリーミング** — 柔軟だが VAD・ウィンドウ管理・遅延制御を全て自前実装。L スコープに対してリスク大
2. **yamabiko-whisper（LocalAgreement-2）** — 低遅延仮説更新は本 spec の追記のみ供給モデルと不整合。VAD 必須で設定が複雑
3. **クラウド STT API** — 要件 2.2 / 9.1 で明示除外

## Notes

- モデル初回取得は HTTPS（HuggingFace ggml 配布）のみ。取得後はオフライン
- **デフォルトモデル**: `ggml-small-q5_0.bin` → **ADR-0004** で `kotoba-whisper-v2.2-ggml-q5_0.bin` → **ADR-0010** で `kotoba-whisper-v2.2-ggml-q8_0.bin` → **ADR-0011** で `kotoba-whisper-v2.2-ggml.bin`（FP16）に置換（ライブラリ判断は本 ADR を維持）
- 判断を覆す場合は新 ADR を作成し、本 ADR の Status を `Superseded by ADR-XXXX` に変更する
