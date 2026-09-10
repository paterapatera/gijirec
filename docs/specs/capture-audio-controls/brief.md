# Brief: capture-audio-controls

## Trigger
スピーカー（システム音声）のみを文字起こししたい場面があるが、現状はマイク入力も常にミックスされる。加えて、推論に渡す際の音量ゲインは固定値で調整されているが、画面上で dB を確認して −18〜−17 dB に手動で合わせたい。

## Problem
会議で自分の発話を拾わず相手の音声だけを転写したいとき、マイクを止める手段がない。音量は内部の固定ゲインに依存しており、環境や会議アプリの音量差をユーザーが目視・手動で合わせられない。

## Desired Outcome
マイクを OFF にしてスピーカー音声のみを転写パイプラインに渡せる。推論直前の音声レベルを画面に dB（dBFS）表示し、手動ゲイン調整で −18〜−17 dB 付近に合わせられる。

## Scope
- **In**:
  - マイク入力の ON/OFF（OFF 時はスピーカー／システム音声のみミックス）
  - 推論 ingest 前の音量レベルの dB 表示（リアルタイムまたは近リアルタイム）
  - 手動ゲイン調整 UI（目標 −18〜−17 dBFS をユーザーが合わせられる）
  - 既存のデバイス選択パネル／キャプチャ再開フローとの整合
- **Out**:
  - OS 側のデバイス音量・システムミキサーの代替
  - 自動 AGC の新設や、固定ゲイン仕様の詳細な置換方針の先取り（requirements で決定）
  - 仮想オーディオデバイス導入
  - 録音ファイルへのエクスポートや話者分離

## Route
- **Path**: C
- **Rationale**: マイク OFF とゲインメーターは別ドメインに見えるが、いずれも「キャプチャ→転写 ingest 前のユーザー制御」として 1 画面・1 spec で収まる単一スコープ。

## Approach
既存の `DeviceSelectorPanel` 周辺または同等のキャプチャ設定 UI にマイクトグルと dB メーター＋ゲインスライダーを追加し、`PcmIngestConsumer` 等の ingest 前処理と接続する。

## Current State
- `audio-device-selection`（完了）: マイク／スピーカー一覧・セッション選択
- `transcribe-volume-normalize`（完了）: 固定ゲイン ×1.25 + ソフトリミット 0.95、UI なし

## Upstream / Downstream
- **Upstream**: none（`transcribe-volume-normalize` は `product.md` で完了済み。ingest 固定ゲインは既存実装を前提に UI 制御を追加）
- **Downstream**: なし（想定）

## Constraints
仮想オーディオデバイスを要求しない。キャプチャ／転写の既存パフォーマンス特性を大きく損なわない。
