# audio-device-selection

- **Surface type**: API / Event
- **Owners / Domains**: audio-device-selection
- **Related ADR**: docs/architecture/adr/ADR-0009-macos-speaker-selection-strategy.md

## Purpose

利用可能なマイク・スピーカー（ループバック対象出力）の一覧取得、セッション内デバイス選択、選択変更通知の Tauri IPC 契約。音声 PCM 形状は `audio-capture-pcm.md` を変更しない。

## Contract

### 型定義

```typescript
/** cpal Device::name()（cpal 0.16 は Device::id() 非公開）。セッション内安定。再起動後の復元は非対象。 */
type AudioDeviceId = string;

type AudioDeviceKind = "input" | "output";

interface AudioDeviceInfo {
  id: AudioDeviceId;
  /** OS が提供する表示名（要件 1.3） */
  name: string;
  kind: AudioDeviceKind;
  /** 当該 kind の OS 既定デバイス */
  is_default: boolean;
}

interface DeviceSelection {
  /** null = OS 既定マイク（要件 2.5–2.6） */
  microphone_id: AudioDeviceId | null;
  /** null = OS 既定出力（ループバック対象、要件 2.5–2.6） */
  speaker_id: AudioDeviceId | null;
}

interface AudioDeviceList {
  inputs: AudioDeviceInfo[];
  outputs: AudioDeviceInfo[];
}
```

### Tauri Commands

| Command | Request | Response | Errors |
|---------|---------|----------|--------|
| `list_audio_devices` | なし | `AudioDeviceList` | `INTERNAL` |
| `get_device_selection` | なし | `DeviceSelection` | なし |
| `set_device_selection` | `DeviceSelection` | `DeviceSelection`（反映後） | `INVALID_DEVICE`, `MACOS_OUTPUT_NOT_DEFAULT`, `INTERNAL` |
| `set_audio_device_ui_visible` | `{ visible: boolean }` | なし | なし |

**`set_device_selection` 挙動**:
1. 指定 ID が現在の一覧に存在することを検証（存在しない場合 `INVALID_DEVICE`）
2. macOS で `speaker_id` が非 null のとき、当該 ID が OS 既定出力と一致しない場合 `MACOS_OUTPUT_NOT_DEFAULT`（ADR-0009）
3. セッション選択状態を更新
4. キャプチャが `capturing` または `starting` のとき `CaptureOrchestrator::restart_with_selection` を呼ぶ（要件 3.3）
5. `audio-device-selection://selection-changed` を emit

**禁止**: 選択デバイスが利用不能なとき別デバイスへサイレントフォールバック（要件 3.4, 4.2）

### Tauri イベント

#### `audio-device-selection://devices-changed`

デバイス一覧が変化したとき（ホットプラグ等）。**デバイス選択 UI が表示されている場合のみ**発行（要件 1.4）。

```typescript
interface AudioDevicesChanged {
  devices: AudioDeviceList;
  timestamp_ms: number;
}
```

#### `audio-device-selection://selection-changed`

```typescript
interface DeviceSelectionChanged {
  selection: DeviceSelection;
  timestamp_ms: number;
}
```

### Command エラー（invoke エラー payload）

| code | 条件 | message_ja 例 | action_ja 例 |
|------|------|---------------|--------------|
| `INVALID_DEVICE` | 一覧に無い ID を指定 | 選択したデバイスが見つかりません | 一覧を更新して別のデバイスを選んでください |
| `MACOS_OUTPUT_NOT_DEFAULT` | macOS で選択スピーカー ≠ OS 既定出力 | 選択したスピーカーがシステムの出力先になっていません | システム設定 → サウンドで出力先を変更するか、一覧から現在の出力先を選んでください |

キャプチャ開始・実行時のデバイスエラーは `audio-capture-status.md` の `audio-capture://error` を使用する。

## Non-goals

- 選択のディスク永続化（アプリ再起動後の復元）
- デバイス一覧・選択内容の外部ネットワーク送信（要件 7.2–3）
- Linux 対応
- PCM チャンク形状の変更

## Changelog

| Date | Change | ADR / rationale |
|------|--------|-----------------|
| 2026-09-06 | `set_audio_device_ui_visible` を追加（UI マウント時のホットプラグ監視 on/off、要件 1.4） | task 5.1 / 6.2 先行 |
| 2026-09-06 | `AudioDeviceId` は cpal 0.16 の `Device::name()` を使用（`Device::id()` 非公開）。同名デバイスはセッション内で衝突しうる。ディスク永続化なし | 実装整合（gijirec-infrastructure `AudioDeviceEnumerator`） |
| 2026-09-06 | 初版 — 一覧・選択 command / 変更イベント | audio-device-selection 設計 |

## Notes

- `list_audio_devices` の `outputs` はループバック取得候補（物理出力デバイス）。macOS では SCK 取得はシステムミックスであり、実際の取得は OS 既定出力経由（ADR-0009）
- セッション `AudioDeviceId` は cpal 0.16 の `Device::name()` をそのまま使用する（公開 API に `Device::id()` が無いため）。`id` と `name` は通常同一文字列。同名デバイスが並存する環境では ID 衝突しうる。選択状態のディスク永続化は行わない
- フロント型ミラー: `src/presentation/hooks/audio-device-types.ts`（命名は実装時に整合）
