use std::sync::{Arc, Mutex};

use gijirec_presentation::application::capture_audio_controls::CaptureAudioControlsService;
use gijirec_presentation::application::capture_audio_controls::IngestSourcePort;
use gijirec_presentation::tauri::capture_audio_controls::IngestLevelSnapshotCache;
use gijirec_presentation::tauri::lifecycle::CaptureProcessingHook;
use gijirec_presentation::transcribe::{
    IngestLevelChangedPayload, IngestLevelEmitter, IngestLevelEventEmitter, PcmIngestConsumer,
};

use crate::capture_processing::CaptureProcessingGate;

use super::port_adapters::ComposeIngestSourcePort;

pub(crate) struct CachingIngestLevelEventEmitter {
    inner: Mutex<Option<Arc<dyn IngestLevelEventEmitter>>>,
    cache: IngestLevelSnapshotCache,
}

impl CachingIngestLevelEventEmitter {
    pub(crate) fn new(cache: IngestLevelSnapshotCache) -> Self {
        Self {
            inner: Mutex::new(None),
            cache,
        }
    }

    pub(crate) fn set_emitter(&self, emitter: Arc<dyn IngestLevelEventEmitter>) {
        *self.inner.lock().expect("lock") = Some(emitter);
    }
}

impl IngestLevelEventEmitter for CachingIngestLevelEventEmitter {
    fn emit_ingest_level(&self, payload: IngestLevelChangedPayload) -> Result<(), String> {
        *self.cache.lock().expect("lock ingest level cache") = Some(
            gijirec_presentation::tauri::capture_audio_controls::IngestLevelSnapshot {
                level_dbfs: payload.level_dbfs,
                timestamp_ms: payload.timestamp_ms,
            },
        );
        if let Some(emitter) = self.inner.lock().expect("lock").as_ref() {
            emitter.emit_ingest_level(payload)?;
        }
        Ok(())
    }
}

/// Applies stored session controls and ingest meter supply on capture lifecycle transitions.
pub(crate) struct CaptureAudioControlsProcessingHook {
    service: Arc<dyn CaptureAudioControlsService>,
    mic_gate: CaptureProcessingGate,
    pcm_ingest: Arc<PcmIngestConsumer>,
    ingest_level_emitter: Arc<IngestLevelEmitter>,
    ingest_level_cache: IngestLevelSnapshotCache,
    ingest_source: ComposeIngestSourcePort,
}

impl CaptureAudioControlsProcessingHook {
    fn sync_live_controls(&self) {
        let controls = self.service.get_state().controls;
        self.mic_gate
            .set_mic_ingest_enabled(controls.mic_ingest_enabled);
        self.pcm_ingest
            .set_ingest_gain_multiplier(controls.manual_ingest_gain);
        let supplying = self
            .ingest_source
            .has_ingestable_audio_source(controls.mic_ingest_enabled);
        self.ingest_level_emitter.set_supplying(supplying);
    }
}

#[cfg(debug_assertions)]
impl CaptureAudioControlsProcessingHook {
    pub(crate) fn pcm_ingest(&self) -> &Arc<PcmIngestConsumer> {
        &self.pcm_ingest
    }

    pub(crate) fn ingest_level_emitter(&self) -> &Arc<IngestLevelEmitter> {
        &self.ingest_level_emitter
    }
}

impl CaptureProcessingHook for CaptureAudioControlsProcessingHook {
    fn on_capture_started(&self) {
        self.sync_live_controls();
    }

    fn on_capture_stopping(&self) {
        self.ingest_level_emitter.set_supplying(false);
        *self
            .ingest_level_cache
            .lock()
            .expect("lock ingest level cache") = None;
    }
}

pub(crate) struct CaptureAudioControlsHookDeps {
    pub(crate) service: Arc<dyn CaptureAudioControlsService>,
    pub(crate) mic_gate: CaptureProcessingGate,
    pub(crate) pcm_ingest: Arc<PcmIngestConsumer>,
    pub(crate) ingest_level_emitter: Arc<IngestLevelEmitter>,
    pub(crate) ingest_level_cache: IngestLevelSnapshotCache,
    pub(crate) ingest_source: ComposeIngestSourcePort,
}

impl CaptureAudioControlsProcessingHook {
    pub(crate) fn new(deps: CaptureAudioControlsHookDeps) -> Self {
        Self {
            service: deps.service,
            mic_gate: deps.mic_gate,
            pcm_ingest: deps.pcm_ingest,
            ingest_level_emitter: deps.ingest_level_emitter,
            ingest_level_cache: deps.ingest_level_cache,
            ingest_source: deps.ingest_source,
        }
    }
}
