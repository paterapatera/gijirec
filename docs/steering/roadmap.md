# Roadmap

## Overview
マイクとスピーカーを仮想デバイスなしで同時に取り込み、ローカル Whisper で低遅延に文字起こしし、その場で手動編集して Markdown 保存できるデスクトップアプリ。

## Scope
- **In**: 二重キャプチャ＋ミキシング、ローカル逐次文字起こし、部分ロック付きエディタ、Markdown 出力、ダブルクリック起動／ウィンドウ閉じで完全終了、モデル取得後オフライン
- **Out**: BlackHole 等の仮想デバイス前提、クラウド音声認識、Python ランタイム、話者分離、Linux 対応

## Constraints
仮想オーディオデバイスを要求しない。会議の裏で OS 全体を極端に重くしない。テキスト追加時に激しいレイアウトシフトや点滅を起こさない。モデル初回取得後はオフラインで全機能が動く。

## Specs (dependency order)
- [x] audio-capture -- マイク＋システム音声を同時取得し 16kHz モノラルへミックス
- [x] whisper-transcribe -- チャンク投入・低遅延ストリーミング・タイムスタンプ付きブロック供給（状態 UI まで完了。転写テキスト表示は transcript-editor へ委譲）
- [x] transcript-editor -- 部分ロック編集・タイムスタンプ維持・Markdown / JSONL 保存。Dependencies: whisper-transcribe
- [x] release-logging -- リリースビルド `--log` 診断ログ永続化。Dependencies: none
- [x] fix-release-transcribe -- release 文字起こしパリティ修正（ModelStore app_data_dir、block-appended ACL、TranscribeStallWatchdog）。Dependencies: release-logging
- [x] audio-device-selection -- マイク/スピーカーデバイス選択・キャプチャ再開。Dependencies: none

## Direct Implementation Candidates
- [x] default-window-size -- `src-tauri/tauri.conf.json` で main ウィンドウ 1000×800（実装済み）
