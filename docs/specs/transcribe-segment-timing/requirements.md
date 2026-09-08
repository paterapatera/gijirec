# 要件定義書

## はじめに

AI 転写の発話区切り（セグメント確定）までの待ち時間が長く、会議中のリアルタイム性が損なわれている。本 spec は既存の Whisper ストリーミング推論パイプラインにおける VAD / 無音判定 / 窓長などの区切り関連パラメータをチューニングし、区切り待ち時間を現状のおおよそ半分に短縮する。モデル変更・エディタ UI 変更・ユーザー向け設定 UI は対象外とする。

## スコープ境界

- **対象範囲**: `TranscribeWorker` の RMS フレーム VAD によるエンドポイント検出定数の調整、調整に伴う回帰テスト更新、実機での区切り速度・品質確認
- **対象外**: モデル変更（kotoba-whisper 以外への切替）、エディタ UI、手入力エリア（`fix-handwriting-input`）、Linux 対応、ユーザー向け設定 UI、フロントエンド変更、`single_segment` の有効化
- **隣接システム・仕様への期待**: `whisper-transcribe-blocks` 契約（追記のみ供給、5 秒遅延目標、TranscriptBlock 形状）は維持する。下流 transcript-editor は変更不要

## 要件

### 要件 1: セグメント区切りの高速化

**目的:** 会議参加者として、発話区切り後に転写テキストが速く UI に現れるようにしたい。その結果、リアルタイム文字起こしの体感遅延が改善される。

#### 受け入れ条件

1. When ユーザーが連続発話を行い、発話区間の末尾で無音が発生したとき、the Transcribe Pipeline shall ベースライン計測時点と比較して、発話区切りから `whisper-transcribe://block-appended` イベント発行までの待ち時間の中央値が **50% 以上短縮** される
2. When ベースライン計測を実施する前に、the Transcribe Pipeline shall 現行の `TRAILING_SILENCE_FRAMES` および `LONG_SILENCE_FRAMES` の値を記録し、調整後の比較基準として保持する
3. The Transcribe Pipeline shall 区切りタイミングの調整を **1 軸ずつ** 行い、同一セッションで `single_segment` / `entropy_thold` / 窓長と VAD 定数を同時に変更しない
4. Where 区切り待ち時間を短縮するための定数変更を行う場合、the Transcribe Pipeline shall まず `TRAILING_SILENCE_FRAMES`（現行 5 = 500 ms）を半分方向（目安 2〜3 フレーム = 200〜300 ms）に調整する

### 要件 2: 転写品質の維持

**目的:** 会議参加者として、区切りが早まっても転写テキストの可読性が保たれるようにしたい。その結果、過剰分割・繰り返し・欠落が実用範囲内に収まる。

#### 受け入れ条件

1. If 区切り定数の調整後に同一フレーズが 3 回以上連続して転写ブロックに出現した場合、the Transcribe Pipeline shall その調整を revert し、別の 1 軸（例: `LONG_SILENCE_FRAMES` のみ）で再試行する
2. While 実機で 3 秒以上の連続日本語発話を行っている場合、the Transcribe Pipeline shall 発話途中で不要なブロック分割を起こさない（1 発話区間が 2 ブロック以上に過剰分割されないことを手動確認で検証）
3. The Transcribe Pipeline shall `set_single_segment(false)` を維持する（日本語繰り返しループ回避のため変更禁止）
4. If 調整後の転写ブロックが `whisper-transcribe-blocks` 契約に違反する（空文字列ブロック、sequence 欠番、既発行ブロックの変更）場合、the Transcribe Pipeline shall その調整を採用しない

### 要件 3: 検証と回帰防止

**目的:** 開発者として、パラメータ調整が品質ゲートを通過し既存テストが壊れないようにしたい。その結果、安全にマージ可能な変更として提供できる。

#### 受け入れ条件

1. When 定数変更をコミットする前に、the Development Process shall `bun run verify` を実行し、全チェックがパスする
2. The Transcribe Worker shall 既存のエンドポイント検出ユニットテスト（trailing silence cut、long-pause short speech、forced cut、skip-to-latest、leading silence trim）を更新後もパスする
3. When 調整セッションを完了する場合、the Development Process shall 変更した定数名・旧値・新値・実機確認結果（区切り速度・品質）を `docs/manual/whisper-transcribe/` に記録する
4. The Transcribe Pipeline shall 発話区切りからブロック供給までの遅延が `whisper-transcribe-blocks` の **5 秒遅延目標** を維持する（調整後も手動 E2E で確認）

### 要件 4: 変更範囲の限定

**目的:** プロダクトオーナーとして、本修正が転写パイプラインの区切りタイミングに限定されるようにしたい。その結果、不要なスコープ拡大を防げる。

#### 受け入れ条件

1. The Transcribe Segment Timing Change shall `src-tauri/crates/gijirec-infrastructure/src/transcribe/transcribe_worker.rs` の VAD / エンドポイント定数を主たる変更対象とする
2. The Transcribe Segment Timing Change shall フロントエンド（`src/presentation/`）に変更を加えない
3. The Transcribe Segment Timing Change shall ユーザー向け設定 UI や runtime 設定ファイルを追加しない（コンパイル時定数のチューニングのみ）
4. Where `entropy_thold` を変更する場合、the Transcribe Segment Timing Change shall `whisper_adapter.rs` で `FullParams` に明示設定し、steering 記載と実装の乖離を解消する（本 spec では VAD 定数を第一手段とし、`entropy_thold` はオプションの第二軸）
