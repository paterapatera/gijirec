//! Integration tests (Testing Strategy Integration 1-5) for whisper-transcribe.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use gijirec_presentation::application::transcribe::block_emitter::BlockEmitter;
use gijirec_presentation::application::transcribe::model_orchestrator::{
    ModelOrchestrator, ModelOrchestratorConfig,
};
use gijirec_presentation::application::transcribe::orchestrator::{
    DefaultTranscribeOrchestrator, TranscribeOrchestrator,
};
use gijirec_presentation::application::transcribe::ports::{
    ModelDownloadProgress, ModelDownloadStatus, ModelDownloaderPort, ModelStorePort,
    TranscribeWorkerPort, WhisperContextPort,
};
use gijirec_presentation::domain::audio::CapturePhase;
use gijirec_presentation::domain::audio::pcm_chunk::{CHUNK_FRAME_COUNT, PcmChunk};
use gijirec_presentation::domain::transcribe::{
    TranscribeError, TranscribePhase, TranscriptBlock, TranscriptBlockConsumer,
    TranscriptConsumerError, TranscriptSegmentSink,
};
use gijirec_presentation::infrastructure::transcribe::{
    SegmentEngine, TranscribeWorker, WhisperSegment,
};
use gijirec_presentation::tauri::pcm_bus::PcmChunkBus;
use gijirec_presentation::transcribe::{
    PcmIngestConsumer, TranscribeEmitError, TranscribeEventEmitter, TranscribeLifecycleHook,
    TranscribeWorkerPortAdapter, TranscriptBlockBus,
};

// ==========================================
// Mocks & Helpers
// ==========================================

#[derive(Default)]
struct RecordingBlockConsumer {
    blocks: Arc<Mutex<Vec<TranscriptBlock>>>,
}

impl TranscriptBlockConsumer for RecordingBlockConsumer {
    fn on_block_appended(&self, block: TranscriptBlock) -> Result<(), TranscriptConsumerError> {
        self.blocks.lock().unwrap().push(block);
        Ok(())
    }
}

#[derive(Default)]
struct RecordingTranscribeEventEmitter {
    phases: Arc<Mutex<Vec<TranscribePhase>>>,
    errors: Arc<Mutex<Vec<TranscribeError>>>,
    progress: Arc<Mutex<Vec<ModelDownloadProgress>>>,
}

impl TranscribeEventEmitter for RecordingTranscribeEventEmitter {
    fn emit_phase_changed(&self, phase: TranscribePhase) -> Result<(), TranscribeEmitError> {
        self.phases.lock().unwrap().push(phase);
        Ok(())
    }

    fn emit_model_progress(
        &self,
        progress: &ModelDownloadProgress,
    ) -> Result<(), TranscribeEmitError> {
        self.progress.lock().unwrap().push(progress.clone());
        Ok(())
    }

    fn emit_error(&self, error: &TranscribeError) -> Result<(), TranscribeEmitError> {
        self.errors.lock().unwrap().push(error.clone());
        Ok(())
    }
}

struct MockStore {
    path: PathBuf,
}

impl ModelStorePort for MockStore {
    fn model_path(&self) -> PathBuf {
        self.path.clone()
    }
    fn verify(&self, _expected: Option<&str>) -> Result<PathBuf, TranscribeError> {
        Ok(self.path.clone())
    }
}

struct MockSequenceDownloader {
    progress_series: Vec<ModelDownloadProgress>,
}

impl ModelDownloaderPort for MockSequenceDownloader {
    fn download(
        &self,
        _url: &str,
        _dest: &std::path::Path,
        on_progress: &mut dyn FnMut(ModelDownloadProgress),
    ) -> Result<(), TranscribeError> {
        for p in &self.progress_series {
            on_progress(p.clone());
        }
        Ok(())
    }
}

struct MockSegmentEngine {
    segments: Vec<WhisperSegment>,
    inference_called: Arc<AtomicBool>,
}

impl SegmentEngine for MockSegmentEngine {
    fn transcribe_pcm(&self, _pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError> {
        self.inference_called.store(true, Ordering::SeqCst);
        Ok(self.segments.clone())
    }

    fn is_loaded(&self) -> bool {
        true
    }
}

// ==========================================
// Integration Tests 1 - 5
// ==========================================

/// Integration Test 1: 合成 PCM → ブロック emit（モック WhisperAdapter / ワーカー結合） (req 2.1, 3.3)
#[test]
fn integration_1_synthetic_pcm_to_block_emission() {
    let bus = Arc::new(TranscriptBlockBus::new());
    let consumer = Arc::new(RecordingBlockConsumer::default());
    let recorded_blocks = Arc::clone(&consumer.blocks);
    bus.register(consumer);

    let emitter = Arc::new(BlockEmitter::new(bus));
    let (mut prod, cons) = rtrb::RingBuffer::<f32>::new(160_000);

    let engine = MockSegmentEngine {
        segments: vec![WhisperSegment {
            text: "こんにちは世界".to_string(),
            start_ms: 1000,
            end_ms: 2500,
        }],
        inference_called: Arc::new(AtomicBool::new(false)),
    };

    let mut worker =
        TranscribeWorker::with_engine(emitter as Arc<dyn TranscriptSegmentSink>, engine);
    worker.attach_pcm_consumer(cons);

    worker.spawn().expect("worker spawn");

    // Feed synthetic 80,000 samples (5 seconds @ 16 kHz) to trigger inference window
    for _ in 0..80_000 {
        let _ = prod.push(0.25);
    }

    // Wait briefly for worker inference loop to pick up and process window
    for _ in 0..50 {
        if !recorded_blocks.lock().unwrap().is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    worker
        .stop_and_join(Duration::from_secs(2))
        .expect("worker stop");

    let blocks = recorded_blocks.lock().unwrap();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].text, "こんにちは世界");
    assert_eq!(blocks[0].sequence, 1);
    assert_eq!(blocks[0].start_timestamp_ms, 1000);
}

/// Integration Test 2: PcmChunkBus 登録 → consumer 経由でワーカーへ到達 (req 1.1)
#[test]
fn integration_2_pcm_chunk_bus_delivers_to_ingest_consumer_and_worker_consumes() {
    let pcm_bus = PcmChunkBus::new();
    let (prod, cons) = rtrb::RingBuffer::<f32>::new(160_000);
    let ingest_consumer = Arc::new(PcmIngestConsumer::new(prod));

    pcm_bus.register(ingest_consumer);

    let bus = Arc::new(TranscriptBlockBus::new());
    let emitter = Arc::new(BlockEmitter::new(bus));
    let inference_called = Arc::new(AtomicBool::new(false));

    let engine = MockSegmentEngine {
        segments: vec![WhisperSegment {
            text: "チャンク受信".to_string(),
            start_ms: 0,
            end_ms: 5000,
        }],
        inference_called: Arc::clone(&inference_called),
    };

    let mut worker =
        TranscribeWorker::with_engine(emitter as Arc<dyn TranscriptSegmentSink>, engine);
    worker.attach_pcm_consumer(cons);
    worker.spawn().expect("worker spawn");

    // Publish 50 chunks of 1600 samples (50 * 1600 = 80,000 samples = 5s) through PcmChunkBus
    for seq in 0..50 {
        let samples = vec![16384_i16; CHUNK_FRAME_COUNT as usize];
        let chunk = PcmChunk::new(seq, samples, seq * 100).expect("chunk");
        pcm_bus.publish(chunk);
    }

    // Wait for worker to consume and invoke engine
    for _ in 0..50 {
        if inference_called.load(Ordering::SeqCst) {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert!(inference_called.load(Ordering::SeqCst));
    worker
        .stop_and_join(Duration::from_secs(2))
        .expect("worker stop");
}

/// Integration Test 3: キャプチャ error イベント → 推論停止 → capturing 復帰で再開 (req 8.3)
#[test]
fn integration_3_capture_error_stops_transcribe_and_resumes_on_capturing() {
    #[derive(Clone)]
    struct MockWorkerPort {
        spawns: Arc<AtomicUsize>,
        stops: Arc<AtomicUsize>,
    }
    impl TranscribeWorkerPort for MockWorkerPort {
        fn spawn(&mut self) -> Result<(), TranscribeError> {
            self.spawns.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn stop_and_join(&mut self, _timeout: Duration) -> Result<(), TranscribeError> {
            self.stops.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct MockCtx;
    impl WhisperContextPort for MockCtx {
        fn load_model(&mut self, _path: &std::path::Path) -> Result<(), TranscribeError> {
            Ok(())
        }
    }

    let worker_spawns = Arc::new(AtomicUsize::new(0));
    let worker_stops = Arc::new(AtomicUsize::new(0));
    let worker = MockWorkerPort {
        spawns: Arc::clone(&worker_spawns),
        stops: Arc::clone(&worker_stops),
    };

    let store = MockStore {
        path: PathBuf::from("/tmp/model.bin"),
    };
    let downloader = MockSequenceDownloader {
        progress_series: vec![],
    };
    let model_orch = Arc::new(Mutex::new(ModelOrchestrator::new(
        store,
        downloader,
        ModelOrchestratorConfig {
            model_url: "http://example.com/model.bin".to_string(),
            expected_sha256: "hash".to_string(),
        },
    )));

    let orch = Arc::new(Mutex::new(DefaultTranscribeOrchestrator::new(
        worker,
        MockCtx,
        model_orch,
        Duration::from_millis(500),
    )));

    // Ensure model so phase becomes Ready
    orch.lock().unwrap().ensure_model().expect("ensure model");
    assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Ready);

    let emitter = Arc::new(RecordingTranscribeEventEmitter::default());
    let hook = TranscribeLifecycleHook::new(
        Arc::clone(&orch) as Arc<Mutex<dyn TranscribeOrchestrator>>,
        emitter.clone(),
    );

    // 1. Capture starts -> Transcribing
    hook.on_capture_phase_changed(CapturePhase::Capturing);
    assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Transcribing);
    assert_eq!(worker_spawns.load(Ordering::SeqCst), 1);

    // 2. Capture error -> Pause / Ready
    hook.on_capture_phase_changed(CapturePhase::Error);
    assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Ready);
    assert_eq!(worker_stops.load(Ordering::SeqCst), 1);
    assert!(!emitter.errors.lock().unwrap().is_empty());

    // 3. Capture resumes -> Transcribing
    hook.on_capture_phase_changed(CapturePhase::Capturing);
    assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Transcribing);
    assert_eq!(worker_spawns.load(Ordering::SeqCst), 2);
}

/// Integration Test 4: stop → ワーカー join 完了、バックグラウンドスレッド残存なし (req 6.5)
#[test]
fn integration_4_stop_joins_transcribe_worker_completely() {
    let bus = Arc::new(TranscriptBlockBus::new());
    let emitter = Arc::new(BlockEmitter::new(bus));
    let (mut prod, cons) = rtrb::RingBuffer::<f32>::new(4096);

    let mut worker = TranscribeWorker::new(emitter as Arc<dyn TranscriptSegmentSink>);
    worker.attach_pcm_consumer(cons);

    let mut adapter = TranscribeWorkerPortAdapter::from_worker(worker);
    adapter.spawn().expect("spawn worker thread");
    assert!(adapter.inner().is_active());

    // Push some audio data into ringbuffer
    for _ in 0..1600 {
        let _ = prod.push(0.0);
    }

    // Stop and join worker
    let res = adapter.stop_and_join(Duration::from_secs(2));
    assert!(res.is_ok());
    assert!(
        !adapter.inner().is_active(),
        "worker thread must not remain active after stop"
    );

    // Repeated stops should also succeed smoothly
    let res2 = adapter.stop_and_join(Duration::from_millis(100));
    assert!(res2.is_ok());
}

/// Integration Test 5: モデルダウンロードモック → model-progress イベント系列 (req 5.1)
#[test]
#[allow(clippy::too_many_lines)]
fn integration_5_model_download_progress_event_series() {
    let progress_events = vec![
        ModelDownloadProgress {
            bytes_downloaded: 0,
            bytes_total: Some(1000),
            percent: Some(0.0),
            status: ModelDownloadStatus::Downloading,
        },
        ModelDownloadProgress {
            bytes_downloaded: 500,
            bytes_total: Some(1000),
            percent: Some(50.0),
            status: ModelDownloadStatus::Downloading,
        },
        ModelDownloadProgress {
            bytes_downloaded: 1000,
            bytes_total: Some(1000),
            percent: Some(100.0),
            status: ModelDownloadStatus::Complete,
        },
    ];

    struct MockStoreMissingThenOk {
        calls: AtomicUsize,
        path: PathBuf,
    }
    impl ModelStorePort for MockStoreMissingThenOk {
        fn model_path(&self) -> PathBuf {
            self.path.clone()
        }
        fn verify(&self, _expected: Option<&str>) -> Result<PathBuf, TranscribeError> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(TranscribeError::ModelNotFound {
                    detail: "missing".to_string(),
                })
            } else {
                Ok(self.path.clone())
            }
        }
    }

    struct MockCtx;
    impl WhisperContextPort for MockCtx {
        fn load_model(&mut self, _path: &std::path::Path) -> Result<(), TranscribeError> {
            Ok(())
        }
    }

    struct DummyWorker;
    impl TranscribeWorkerPort for DummyWorker {
        fn spawn(&mut self) -> Result<(), TranscribeError> {
            Ok(())
        }
        fn stop_and_join(&mut self, _timeout: Duration) -> Result<(), TranscribeError> {
            Ok(())
        }
    }

    let store = MockStoreMissingThenOk {
        calls: AtomicUsize::new(0),
        path: PathBuf::from("/tmp/model.bin"),
    };
    let downloader = MockSequenceDownloader {
        progress_series: progress_events,
    };
    let model_orch = Arc::new(Mutex::new(ModelOrchestrator::new(
        store,
        downloader,
        ModelOrchestratorConfig {
            model_url: "https://example.com/model.bin".to_string(),
            expected_sha256: "hash".to_string(),
        },
    )));

    let emitter = Arc::new(RecordingTranscribeEventEmitter::default());
    let emitter_for_cb = emitter.clone();

    let mut orch = DefaultTranscribeOrchestrator::new(
        DummyWorker,
        MockCtx,
        model_orch,
        Duration::from_millis(500),
    );

    orch.set_model_progress_callback(Box::new(move |p| {
        let _ = emitter_for_cb.emit_model_progress(&p);
    }));

    orch.ensure_model().expect("ensure model");

    let received_progress = emitter.progress.lock().unwrap().clone();
    assert_eq!(received_progress.len(), 4); // 3 download + 1 verifying
    assert_eq!(received_progress[0].percent, Some(0.0));
    assert_eq!(received_progress[1].percent, Some(50.0));
    assert_eq!(received_progress[2].percent, Some(100.0));
    assert_eq!(received_progress[3].status, ModelDownloadStatus::Verifying);
}
