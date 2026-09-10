# Roadmap

## Overview
マイクとスピーカーを仮想デバイスなしで同時に取り込み、ローカル Whisper で約 30 秒間隔のバッチ文字起こしし、その場で手動編集して Markdown 保存できるデスクトップアプリ。

## Scope
- **In**: 二重キャプチャ＋ミキシング、ローカルバッチ文字起こし（30 秒間隔）、部分ロック付きエディタ、Markdown 出力、ダブルクリック起動／ウィンドウ閉じで完全終了、モデル取得後オフライン
- **Out**: BlackHole 等の仮想デバイス前提、クラウド音声認識、Python ランタイム、話者分離、Linux 対応

## Constraints
仮想オーディオデバイスを要求しない。会議の裏で OS 全体を極端に重くしない。テキスト追加時に激しいレイアウトシフトや点滅を起こさない。モデル初回取得後はオフラインで全機能が動く。

## Planned Specs

新規 spec は `/sdd-discovery` 経由で起票し、依存順でここに `[ ]` として追記する。完了済み feature の履歴は `docs/steering/product.md` の Implementation Phasing 表を参照。

（未着手の spec はなし）

## 完了状態のルール

- **完了**: `docs/specs/<feature>/tasks.md` が全 `[x]`（spec 削除後は `docs/steering/product.md` の実装フェーズ表とコードで確認）
- **`spec.json` の `phase: tasks-approved` は完了と矛盾しない**（phase クローズは別メタデータ）
- **roadmap には未実装 feature のみ載せる** — 完了した spec は roadmap から削除し、`product.md` に残す
- Direct Implementation は設定ファイル（例: `tauri.conf.json`）を grep してから `[x]` にする
- **アーカイブ**: 完了 spec は Implementation Notes 昇格・`docs/manual/` 移設後、人間が週次で `docs/specs/<feature>/` を削除（手順は `docs/steering/structure.md` の Spec ライフサイクル）

---
_updated_at: 2026-09-10（capture-audio-controls 完了・roadmap から除去）_
