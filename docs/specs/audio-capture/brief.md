# Brief: audio-capture

## Trigger
製品のファーストステップとして、Tauri(Rust) でマイクとスピーカーを同時に拾ってミキシングするキャプチャ部から着手する依頼。

## Problem
Web 会議中に自分の声と相手／PC 音声を、仮想オーディオデバイスなしで同時に取れない。取れても後段の文字起こしに渡せる 1 本の PCM にならない。

## Desired Outcome
ダブルクリックで立ち上がる Tauri アプリが、マイクとシステム音声を同時キャプチャし、適切なバランスで 16kHz モノラル PCM にリアルタイム合成する。ウィンドウを閉じるとキャプチャも止まる。

## Scope
- **In**: 仮想デバイスなしの二重取り込み（Mac: ScreenCaptureKit 等 / Windows: WASAPI ループバック等）、16kHz / 16bit / モノラルへの変換・合成、Tauri ホストの起動、ウィンドウ閉じ／Cmd+Q・Alt+F4 でのキャプチャ停止
- **Out**: Whisper 推論、エディタ、Markdown 出力、モデルダウンロード、Linux

## Route
- **Path**: D
- **Rationale**: 製品全体が複数ドメインにまたがるため、キャプチャを先頭 spec として切り出す。

## Approach
Tauri（Rust）＋ Web フロント。OS ネイティブのループバックでシステム音声を取る。cpal 等でデバイスをフックし、内部でミックスする。

## Current State
緑地。実装・既存 spec なし。

## Upstream / Downstream
- Upstream: なし
- Downstream: whisper-transcribe

## Constraints
仮想オーディオデバイスを要求しない。会議の裏で極端に OS を重くしない。
