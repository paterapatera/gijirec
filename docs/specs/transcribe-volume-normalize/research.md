# Gap Analysis: transcribe-volume-normalize

## Summary

Brownfield extension to the existing audio → transcribe pipeline. The mixer (`mixer.rs`) already normalizes to −20 dBFS; this feature adds **transcribe-path-only** gain after `PcmChunkBus` and before the transcribe rtrb.

## Existing Implementation

| Component | Path | Relevance |
|-----------|------|-----------|
| `DefaultAudioMixer` | `src-tauri/crates/gijirec-application/src/capture/mixer.rs` | `TARGET_RMS=0.1`, `MAX_GAIN=4.0`, `SOFT_LIMIT=0.95`, `NOISE_GATE_RMS=0.008` — **unchanged** |
| `PcmIngestConsumer` | `src-tauri/crates/gijirec-presentation/src/transcribe/pcm_ingest_consumer.rs` | i16→f32 conversion only; **preferred insertion point** |
| `TranscribeWorker` | `src-tauri/crates/gijirec-infrastructure/src/transcribe/transcribe_worker.rs` | `SILENCE_RMS_THRESHOLD=0.008`, window RMS before inference |
| `WhisperCppAdapter` | `src-tauri/crates/gijirec-infrastructure/src/transcribe/whisper_adapter.rs` | No preprocessing |
| Wiring | `src-tauri/src/compose.rs` | Registers ingest consumer, observability callbacks |
| RMS logging | `src-tauri/src/transcribe_observability.rs`, `observability.rs` | `transcribe_window_rms_dbfs`, ingest summary fields |

## Gap

- No transcribe-path gain between mixer output and Whisper inference.
- Comfortable OS volume yields ~−23 dBFS at inference window; target is −18〜−17 dBFS (~×1.45).

## Integration Constraints

- `NOISE_GATE_RMS` (mixer) == `SILENCE_RMS_THRESHOLD` (worker) == `0.008` — gain must not push sub-threshold noise above skip threshold spuriously.
- Peak headroom ~−12 dBFS before gain; `SOFT_LIMIT=0.95` prevents clipping.
- Layering: gain belongs in `gijirec-presentation` (ingest) or `gijirec-infrastructure` (worker pre-inference); presentation ingest is lower blast radius.

## Recommended Approach

Apply `TRANSCRIBE_INGEST_GAIN = 1.45` + `soft_limit(0.95)` in `pcm_ingest_consumer.rs` sample loop (after `/32768.0`). Measure RMS in worker **after** ingest gain (existing window RMS path).

## Out of Scope (confirmed)

- Mixer `TARGET_RMS` change (affects all paths).
- OS volume control, AGC, settings UI.
