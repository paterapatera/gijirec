//! Composition root: orchestrator, pipeline hold, transcribe wiring, and lifecycle helpers.

use std::sync::{Arc, Mutex};

use gijirec_presentation::application::capture::orchestrator::{
    CaptureOrchestrator, DefaultCaptureOrchestrator, MicCapturePort, SystemAudioCapturePort,
};
use gijirec_presentation::application::transcribe::block_emitter::BlockEmitter;
use gijirec_presentation::application::transcribe::model_orchestrator::{
    ModelOrchestrator, ModelOrchestratorConfig,
};
use gijirec_presentation::application::transcribe::orchestrator::{
    DefaultTranscribeOrchestrator, TranscribeOrchestrator,
};
use gijirec_presentation::domain::transcribe::TranscriptSegmentSink;
use gijirec_presentation::infrastructure::transcribe::{
    ModelDownloader, ModelStore, TranscribeWorker,
};
use gijirec_presentation::transcribe::lifecycle_hook::DEFAULT_TRANSCRIBE_STOP_TIMEOUT;
use gijirec_presentation::transcribe::{
    ModelDownloaderPortAdapter, ModelStorePortAdapter, PcmIngestConsumer, TranscribeEventEmitter,
    TranscribeLifecycleHook, TranscribeWorkerPortAdapter, TranscriptBlockBus,
    WhisperContextPortAdapter,
};

use crate::capture_ports::CaptureStreamHandles;
use crate::capture_processing::CapturePipelineState;

/// Shared model acquisition handle used without holding the transcribe orchestrator lock.
pub(crate) type SharedModelOrchestrator =
    Arc<Mutex<ModelOrchestrator<ModelStorePortAdapter, ModelDownloaderPortAdapter>>>;

/// Fully composed capture and transcribe stack ready for Tauri lifecycle injection.
pub(crate) struct ComposedCapture {
    pub orchestrator: Box<dyn CaptureOrchestrator>,
    pub pipeline: CapturePipelineState,
    pub transcribe_lifecycle: Arc<TranscribeLifecycleHook>,
    pub transcribe_bus: Arc<TranscriptBlockBus>,
    pub transcribe_orchestrator: Arc<Mutex<dyn TranscribeOrchestrator>>,
    pub model_orchestrator: SharedModelOrchestrator,
}

/// Default HuggingFace download URL and SHA-256 for kotoba-whisper-v2.2 GGML (Q5_0).
pub(crate) const DEFAULT_WHISPER_MODEL_URL: &str = "https://huggingface.co/kenrouse/kotoba-whisper-v2.2-ggml/resolve/main/kotoba-whisper-v2.2-ggml-q5_0.bin";
pub(crate) const DEFAULT_WHISPER_MODEL_SHA256: &str =
    "4a3b92192b5d3578ff854a5876213e2e27af0c2d357492c2d14271e82c303658";

/// Builds the production capture and transcribe stack with platform and whisper adapters.
pub(crate) fn build_capture_stack() -> ComposedCapture {
    let (streams, mic, system) = CaptureStreamHandles::new_pair();
    let app_data = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("gijirec");
    compose_with_ports_and_transcribe(
        mic,
        system,
        streams,
        TranscribeComposeConfig {
            app_data_dir: app_data,
            model_config: ModelOrchestratorConfig {
                model_url: DEFAULT_WHISPER_MODEL_URL.to_string(),
                expected_sha256: DEFAULT_WHISPER_MODEL_SHA256.to_string(),
            },
        },
    )
}

/// Extra parameters for initializing the transcribe stack in composition root.
pub(crate) struct TranscribeComposeConfig {
    pub app_data_dir: std::path::PathBuf,
    pub model_config: ModelOrchestratorConfig,
}

/// Builds capture + transcribe stack from injectable ports and data directories.
pub(crate) fn compose_with_ports_and_transcribe<M, S>(
    mic: M,
    system: S,
    streams: CaptureStreamHandles,
    config: TranscribeComposeConfig,
) -> ComposedCapture
where
    M: MicCapturePort + 'static,
    S: SystemAudioCapturePort + 'static,
{
    let orchestrator = Box::new(DefaultCaptureOrchestrator::new(mic, system));
    let pipeline = CapturePipelineState::new(streams);

    // 1. Set up PCM buffer between IngestConsumer and TranscribeWorker (30 s @ 16 kHz = 480k)
    let (pcm_prod, pcm_cons) = rtrb::RingBuffer::<f32>::new(480_000);
    let mut pcm_ingest = PcmIngestConsumer::new(pcm_prod);
    pcm_ingest.set_sequence_gap_callback(Arc::new(|from, to| {
        gijirec_presentation::transcribe::observability::log_pcm_sequence_gaps(from, to);
    }));
    let pcm_ingest = Arc::new(pcm_ingest);
    pipeline.pcm_bus.register(pcm_ingest);

    // 2. Set up TranscriptBlockBus with drop logging
    let block_bus = TranscriptBlockBus::new();
    block_bus.set_drop_callback(Arc::new(|drops| {
        gijirec_presentation::transcribe::observability::log_block_buffer_drop(drops);
    }));
    let block_bus = Arc::new(block_bus);

    // 3. Set up BlockEmitter as sink for TranscribeWorker
    let block_emitter = Arc::new(BlockEmitter::new(Arc::clone(&block_bus)));

    // 4. Set up TranscribeWorker with latency metric callback
    let mut worker =
        TranscribeWorker::new(Arc::clone(&block_emitter) as Arc<dyn TranscriptSegmentSink>);
    worker.attach_pcm_consumer(pcm_cons);
    worker.set_inference_latency_callback(Arc::new(|ms| {
        gijirec_presentation::transcribe::observability::log_inference_latency(ms);
    }));
    let worker_adapter = TranscribeWorkerPortAdapter::from_worker(worker);

    // 5. Set up ModelStore and ModelDownloader adapters
    let store = ModelStore::new(config.app_data_dir);
    let store_adapter = ModelStorePortAdapter::new(store);
    let downloader = ModelDownloader::new().expect("HTTPS client initialization");
    let downloader_adapter = ModelDownloaderPortAdapter::new(downloader).expect("adapter");
    let model_orchestrator = Arc::new(Mutex::new(ModelOrchestrator::new(
        store_adapter,
        downloader_adapter,
        config.model_config,
    )));

    // 6. Set up Context adapter
    let context_adapter = WhisperContextPortAdapter::new();

    // 7. Compose DefaultTranscribeOrchestrator
    let transcribe_orchestrator = Arc::new(Mutex::new(DefaultTranscribeOrchestrator::new(
        worker_adapter,
        context_adapter,
        Arc::clone(&model_orchestrator),
        DEFAULT_TRANSCRIBE_STOP_TIMEOUT,
    )));

    // 8. Set up Noop/Dummy TranscribeEventEmitter for initial lifecycle hook (updated when Tauri sets up)
    struct MockEmitter;
    impl TranscribeEventEmitter for MockEmitter {
        fn emit_phase_changed(
            &self,
            phase: gijirec_presentation::domain::transcribe::TranscribePhase,
        ) -> Result<(), gijirec_presentation::transcribe::TranscribeEmitError> {
            gijirec_presentation::transcribe::observability::log_phase_transition(phase);
            Ok(())
        }
        fn emit_model_progress(
            &self,
            _progress: &gijirec_presentation::application::transcribe::ModelDownloadProgress,
        ) -> Result<(), gijirec_presentation::transcribe::TranscribeEmitError> {
            Ok(())
        }
        fn emit_error(
            &self,
            error: &gijirec_presentation::domain::transcribe::TranscribeError,
        ) -> Result<(), gijirec_presentation::transcribe::TranscribeEmitError> {
            gijirec_presentation::transcribe::observability::log_transcribe_error(error);
            Ok(())
        }
    }
    let dummy_emitter = Arc::new(MockEmitter);
    let transcribe_lifecycle = Arc::new(TranscribeLifecycleHook::new(
        Arc::clone(&transcribe_orchestrator) as Arc<Mutex<dyn TranscribeOrchestrator>>,
        dummy_emitter,
    ));

    ComposedCapture {
        orchestrator,
        pipeline,
        transcribe_lifecycle,
        transcribe_bus: block_bus,
        transcribe_orchestrator,
        model_orchestrator,
    }
}

/// Builds capture stack with default dummy transcribe stack for testing.
#[cfg(test)]
pub(crate) fn compose_with_ports<M, S>(
    mic: M,
    system: S,
    streams: CaptureStreamHandles,
) -> ComposedCapture
where
    M: MicCapturePort + 'static,
    S: SystemAudioCapturePort + 'static,
{
    compose_with_ports_and_transcribe(
        mic,
        system,
        streams,
        TranscribeComposeConfig {
            app_data_dir: std::env::temp_dir().join("gijirec_test"),
            model_config: ModelOrchestratorConfig {
                model_url: "".to_string(),
                expected_sha256: "".to_string(),
            },
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_presentation::domain::audio::{CaptureError, CapturePhase};
    use std::sync::atomic::{AtomicU8, Ordering};

    struct TrackingMic {
        order: Arc<AtomicU8>,
    }

    struct TrackingSystem {
        order: Arc<AtomicU8>,
    }

    impl MicCapturePort for TrackingMic {
        fn open(&mut self) -> Result<(), CaptureError> {
            self.order.store(1, Ordering::SeqCst);
            Ok(())
        }

        fn close(&mut self) {
            self.order.store(0, Ordering::SeqCst);
        }

        fn is_open(&self) -> bool {
            self.order.load(Ordering::SeqCst) >= 1
        }
    }

    impl SystemAudioCapturePort for TrackingSystem {
        fn open(&mut self) -> Result<(), CaptureError> {
            let mic_first = self.order.load(Ordering::SeqCst) == 1;
            if !mic_first {
                return Err(CaptureError::Internal {
                    detail: "system opened before mic".to_string(),
                });
            }
            self.order.store(2, Ordering::SeqCst);
            Ok(())
        }

        fn close(&mut self) {
            self.order.store(0, Ordering::SeqCst);
        }

        fn is_open(&self) -> bool {
            self.order.load(Ordering::SeqCst) == 2
        }
    }

    #[test]
    fn compose_holds_pipeline_components() {
        let (streams, _mic, _system) = CaptureStreamHandles::new_pair();
        let order = Arc::new(AtomicU8::new(0));
        let mic = TrackingMic {
            order: Arc::clone(&order),
        };
        let system = TrackingSystem {
            order: Arc::clone(&order),
        };
        let composed = compose_with_ports(mic, system, streams);

        assert_eq!(composed.orchestrator.phase(), CapturePhase::Idle);
        assert!(!composed.pipeline.processing_is_active());
    }

    /// Hardware integration test for Windows WASAPI loopback + mic (ignored on CI).
    #[test]
    #[ignore = "CI: requires Windows mic permission and default WASAPI loopback output; run with --ignored on local hardware"]
    fn integration_mic_and_wasapi_loopback_reach_capturing_on_hardware() {
        let composed = build_capture_stack();
        assert_eq!(composed.orchestrator.phase(), CapturePhase::Idle);
    }

    /// Hardware integration test for macOS ScreenCaptureKit + mic (ignored on CI).
    #[test]
    #[ignore = "CI: requires macOS mic permission and ScreenCaptureKit screen recording permission; run with --ignored on local hardware"]
    fn integration_mic_and_sck_reach_capturing_on_hardware() {
        let composed = build_capture_stack();
        assert_eq!(composed.orchestrator.phase(), CapturePhase::Idle);
    }
}
