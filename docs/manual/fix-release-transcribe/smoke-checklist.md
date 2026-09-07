# Release EXE 手動 smoke

`cargo tauri build` 後の配布 EXE で確認する。`--log` は [release-logging 運用手順](../release-logging/operations.md) どおり。

1. clean `gen` のうえ `cargo tauri build` し、生成 EXE を `--log` 付きで起動する。
2. モデル DL 完了後、ログと UI の両方で transcribe phase が `ready` になる。
3. キャプチャ開始後、5 秒以内に `whisper-transcribe://block-appended` でエディタへ追記される。
4. キャプチャ停止後、phase が `ready` に戻り、既存ブロックは保持される。
5. モデル取得済みの状態でオフラインでも転写が継続する。

横断チェック（dev / release パリティ）は [docs/steering/testing.md](../../steering/testing.md) の「リリース vs dev パリティ」を参照。
