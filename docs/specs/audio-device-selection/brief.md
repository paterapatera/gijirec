# Brief: audio-device-selection

## Trigger
複数のマイク・スピーカーがある環境で、使用するデバイスを選べない。デフォルトデバイス固定のため意図しない入力が使われる。

## Problem
ユーザーがマイクとスピーカー（ループバック）を明示的に選べず、会議キャプチャの品質や安定性に影響する。

## Desired Outcome
UI でマイクとスピーカー（ループバック）を選択し、選択したデバイスでキャプチャできる。

## Scope
- **In**: 利用可能デバイスの一覧取得、マイク／スピーカー選択 UI、選択デバイスでのキャプチャ
- **Out**: 仮想デバイス作成、Linux 対応、デバイスごとの詳細チューニング（ゲイン等）

## Route
- **Path**: D
- **Rationale**: 既存 `audio-capture` は実装完了済みのため、デバイス選択機能を独立 spec として切り出す。

## Approach
既存キャプチャスタック（cpal / WASAPI 等）を拡張し、フロントにデバイス選択 UI を追加する。

## Current State
`audio-capture` はデフォルトデバイスでの二重キャプチャが実装済み。デバイス選択 UI は未実装。

## Upstream / Downstream
- Upstream: なし
- Downstream: なし

## Constraints
仮想オーディオデバイスを要求しない。会議の裏で OS を極端に重くしない。
