# 要件定義書

## はじめに

gijirec はマイクとシステム音声をミキシングし、ローカル Whisper で約 30 秒間隔のバッチ文字起こしを行う。現状、快適な OS 音量では推論窓 RMS が約 −23 dBFS となり認識精度が低下する一方、OS 音量を上げると約 −18.7 dBFS 付近で精度が向上する。本 spec は **転写パス専用** の音量正規化を追加し、OS 音量に依存せず Whisper 入力を −18〜−17 dBFS 付近へ安定させる。

## スコープ境界

- **対象範囲**: PCM ミキサー出力から Whisper 推論直前までの転写専用ゲイン適用、ソフトリミッター、静かな入力へのゲイン上限内の正規化、既存 RMS ログによる効果検証、v1 固定パラメータ
- **対象外**: OS / デバイス音量の自動制御、モニター出力の快適音量調整、AGC / コンプレッサの本格導入、ユーザー向け転写感度スライダー UI
- **隣接システム・仕様への期待**: `audio-capture` のミキサー（`TARGET_RMS = 0.1`、−20 dBFS）は変更しない（代替案としての `TARGET_RMS` 引き上げは本 spec の非推奨経路）。`whisper-transcribe` の 30 秒バッチ窓・無音スキップ閾値 `0.008` の契約を維持する

## 要件

### 要件 1: 転写パス専用ゲイン適用

**目的:** ユーザーとして、OS 音量を上げなくても Whisper が安定して認識できる入力レベルを得たい。その結果、機材差に左右されず転写精度が向上する。

#### 受け入れ条件

1. When PCM チャンクが転写 ingest 経路（`PcmIngestConsumer`）に到達する, the gijirec transcribe path shall モニター／ミキサー出力に影響を与えず、転写用 rtrb へ push する前に転写専用の固定ゲインを適用する
2. The gijirec transcribe path shall 転写専用ゲインの初期値を **×1.45**（許容範囲 ×1.4〜1.5）とし、推論窓 RMS が **−18〜−17 dBFS** 付近（線形 RMS 約 0.12〜0.13）を目標とする
3. While 転写専用ゲインを適用する, the gijirec transcribe path shall 各サンプルにソフトリミッター（上限 **0.95**）を適用し、クリッピングを防止する
4. The gijirec transcribe path shall `DefaultAudioMixer` の `TARGET_RMS`（0.1 / −20 dBFS）およびミキサー出力経路を変更しない

### 要件 2: 静かな入力と無音スキップの整合

**目的:** ユーザーとして、静かな会議でも転写が起動し続け、過大ゲインによるノイズ増幅を避けたい。その結果、無音スキップと実運用のバランスが保たれる。

#### 受け入れ条件

1. While 入力 RMS が無音スキップ閾値（**0.008**）未満である, the gijirec transcribe path shall ゲイン適用後も `transcribe_worker` の無音スキップ判定（`SILENCE_RMS_THRESHOLD = 0.008`）と整合するよう、ゲイン後 RMS が閾値を超えないよう設計する（ゲイン前が閾値未満ならゲイン後もスキップ対象のまま）
2. If 静かな入力で目標 RMS に到達するには過大ゲインが必要である, the gijirec transcribe path shall 既存 `MAX_GAIN` 相当の上限（現行ミキサー **4.0** / +12 dB）を超えない転写専用ゲイン上限を適用する
3. When 転写専用ゲインが適用される, the gijirec transcribe worker shall 推論窓 RMS をゲイン適用後の PCM に基づいて計測し、既存の `transcribe_window_rms` / `transcribe_window_rms_dbfs` ログに反映する

### 要件 3: 可観測性と検証

**目的:** 開発者として、音量正規化の効果をログで確認したい。その結果、実機チューニングと regress 検知が可能になる。

#### 受け入れ条件

1. The gijirec transcribe path shall 既存の `transcribe_window_rms_dbfs` および `transcribe_pcm_ingest_*_rms_dbfs` ログフィールドを維持し、ゲイン適用後のレベルを反映する
2. The gijirec transcribe path shall PCM 生データおよび転写本文をログに出力しない（現行の診断ログ契約を維持する）
3. When 快適な OS 音量（実測で推論窓 RMS が約 −23 dBFS となる条件）でキャプチャする, the gijirec transcribe path shall ゲイン適用後の `transcribe_window_rms_dbfs` が **−18〜−17 dBFS** 付近に収まることを手動検証で確認できる

### 要件 4: v1 固定パラメータ

**目的:** ユーザーとして、設定 UI を増やさずに改善を享受したい。その結果、実装と運用が単純に保たれる。

#### 受け入れ条件

1. The gijirec transcribe path shall v1 では転写ゲイン・目標 RMS をコード内固定値として提供し、設定 UI や IPC によるユーザー調整を行わない
2. Where 将来のチューニングが必要になる, the gijirec transcribe path shall ゲイン定数を単一の名前付き定数（例: `TRANSCRIBE_INGEST_GAIN`）に集約し、変更箇所を限定する
