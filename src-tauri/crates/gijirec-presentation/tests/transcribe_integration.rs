//! Integration tests (Testing Strategy Integration 1-5) for whisper-transcribe.

use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};

use gijirec_presentation::application::transcribe::block_emitter::BlockEmitter;
use gijirec_presentation::application::transcribe::model_orchestrator::ModelOrchestrator;
use gijirec_presentation::application::transcribe::orchestrator::{
    DefaultTranscribeOrchestrator, TranscribeOrchestrator,
};
use gijirec_presentation::application::transcribe::ports::{
    ModelDownloadProgress, ModelDownloadStatus, ModelStorePort,
};
use gijirec_presentation::domain::audio::CapturePhase;
use gijirec_presentation::domain::audio::pcm_chunk::CHUNK_FRAME_COUNT;
use gijirec_presentation::domain::transcribe::{
    TranscribeError, TranscribeErrorCode, TranscribePhase, TranscriptBlock, TranscriptSegmentSink,
};
use gijirec_presentation::infrastructure::transcribe::{TranscribeWorker, WhisperSegment};
use gijirec_presentation::tauri::pcm_bus::PcmChunkBus;
use gijirec_presentation::transcribe::test_support::{
    BATCH_WINDOW_SAMPLES, CHUNKS_PER_BATCH, CountingBatchEngine, DeferredPlaceholderStore,
    FailOnceBatchEngine, InjectableMockStore, MockEngineWorkerPort, MockFailingDownloader,
    MockSegmentEngine, MockSequenceDownloader, MockStore, MockWorkerPort, NoopTranscribeWorkerPort,
    NoopWhisperContextPort, RecordingTranscribeEventEmitter, assert_contiguous_sequences,
    create_temp_model_file, publish_speech_pcm, recording_block_bus, setup_batch_pipeline,
    spawn_transcribe_worker, stop_batch_worker_and_take_blocks, wait_for_block_count,
    wait_for_blocks,
};
use gijirec_presentation::transcribe::{
    PcmIngestConsumer, TranscribeEventEmitter, TranscribeLifecycleHook, TranscribeWorkerPort,
    TranscribeWorkerPortAdapter,
};

// ==========================================
// Integration Tests 1 - 5
// ==========================================

/// Integration Test 1: 合成 PCM → ブロック emit（モック WhisperAdapter / ワーカー結合） (req 2.1, 3.3)
#[test]
fn integration_1_synthetic_pcm_to_block_emission() {
    let (bus, recorded_blocks) = recording_block_bus();
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

    let mut worker = spawn_transcribe_worker(emitter, engine, cons);

    for _ in 0..32_000 {
        let _ = prod.push(0.25);
    }
    for _ in 0..(13 * CHUNK_FRAME_COUNT as usize) {
        let _ = prod.push(0.0);
    }

    for _ in 0..50 {
        if !recorded_blocks.lock().unwrap().is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    worker
        .stop_and_join(std::time::Duration::from_secs(2))
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

    let (bus, _) = recording_block_bus();
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

    let mut worker = spawn_transcribe_worker(emitter, engine, cons);

    publish_speech_pcm(&pcm_bus, CHUNKS_PER_BATCH, 0);

    for _ in 0..100 {
        if inference_called.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert!(inference_called.load(Ordering::SeqCst));
    worker
        .stop_and_join(std::time::Duration::from_secs(2))
        .expect("worker stop");
}

/// Integration Test 3: キャプチャ error イベント → 推論停止 → capturing 復帰で再開 (req 8.3)
#[test]
fn integration_3_capture_error_stops_transcribe_and_resumes_on_capturing() {
    let worker_spawns = Arc::new(AtomicUsize::new(0));
    let worker_stops = Arc::new(AtomicUsize::new(0));
    let worker = MockWorkerPort::new(Arc::clone(&worker_spawns), Arc::clone(&worker_stops));

    let store = MockStore {
        path: PathBuf::from("/tmp/model.bin"),
    };
    let downloader = MockSequenceDownloader {
        progress_series: vec![],
    };
    let model_orch = Arc::new(Mutex::new(ModelOrchestrator::new(store, downloader)));

    let orch = Arc::new(Mutex::new(DefaultTranscribeOrchestrator::new(
        worker,
        NoopWhisperContextPort,
        model_orch,
        std::time::Duration::from_millis(500),
    )));

    orch.lock().unwrap().ensure_model().expect("ensure model");
    assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Ready);

    let emitter = Arc::new(RecordingTranscribeEventEmitter::default());
    let hook = TranscribeLifecycleHook::new(
        Arc::clone(&orch) as Arc<Mutex<dyn TranscribeOrchestrator>>,
        emitter.clone(),
    );

    hook.on_capture_phase_changed(CapturePhase::Capturing);
    assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Transcribing);
    assert_eq!(worker_spawns.load(Ordering::SeqCst), 1);

    hook.on_capture_phase_changed(CapturePhase::Error);
    assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Ready);
    assert_eq!(worker_stops.load(Ordering::SeqCst), 1);
    assert!(!emitter.errors.lock().unwrap().is_empty());

    hook.on_capture_phase_changed(CapturePhase::Capturing);
    assert_eq!(orch.lock().unwrap().phase(), TranscribePhase::Transcribing);
    assert_eq!(worker_spawns.load(Ordering::SeqCst), 2);
}

/// Integration Test 4: stop → ワーカー join 完了、バックグラウンドスレッド残存なし (req 6.5)
#[test]
fn integration_4_stop_joins_transcribe_worker_completely() {
    let (bus, _) = recording_block_bus();
    let emitter = Arc::new(BlockEmitter::new(bus));
    let (mut prod, cons) = rtrb::RingBuffer::<f32>::new(4096);

    let mut worker = TranscribeWorker::new(emitter as Arc<dyn TranscriptSegmentSink>);
    worker.attach_pcm_consumer(cons);

    let mut adapter = TranscribeWorkerPortAdapter::from_worker(worker);
    adapter.spawn().expect("spawn worker thread");
    assert!(adapter.inner().is_active());

    for _ in 0..1600 {
        let _ = prod.push(0.0);
    }

    let res = adapter.stop_and_join(std::time::Duration::from_secs(2));
    assert!(res.is_ok());
    assert!(
        !adapter.inner().is_active(),
        "worker thread must not remain active after stop"
    );

    let res2 = adapter.stop_and_join(std::time::Duration::from_millis(100));
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
        fn model_path_for(
            &self,
            _variant: gijirec_domain::transcribe::WhisperModelVariant,
        ) -> PathBuf {
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
        fn verify_variant(
            &self,
            _variant: gijirec_domain::transcribe::WhisperModelVariant,
            expected: Option<&str>,
        ) -> Result<PathBuf, TranscribeError> {
            self.verify(expected)
        }
        fn file_exists(&self, _variant: gijirec_domain::transcribe::WhisperModelVariant) -> bool {
            false
        }
    }

    let store = MockStoreMissingThenOk {
        calls: AtomicUsize::new(0),
        path: PathBuf::from("/tmp/model.bin"),
    };
    let downloader = MockSequenceDownloader {
        progress_series: progress_events,
    };
    let model_orch = Arc::new(Mutex::new(ModelOrchestrator::new(store, downloader)));

    let emitter = Arc::new(RecordingTranscribeEventEmitter::default());
    let emitter_for_cb = emitter.clone();

    let mut orch = DefaultTranscribeOrchestrator::new(
        NoopTranscribeWorkerPort,
        NoopWhisperContextPort,
        model_orch,
        std::time::Duration::from_millis(500),
    );

    orch.set_model_progress_callback(Box::new(move |p| {
        let _ = emitter_for_cb.emit_model_progress(&p);
    }));

    orch.ensure_model().expect("ensure model");

    let received_progress = emitter.progress.lock().unwrap().clone();
    assert_eq!(received_progress.len(), 4);
    assert_eq!(received_progress[0].percent, Some(0.0));
    assert_eq!(received_progress[1].percent, Some(50.0));
    assert_eq!(received_progress[2].percent, Some(100.0));
    assert_eq!(received_progress[3].status, ModelDownloadStatus::Verifying);
}

// ==========================================
// Integration Tests 6 - 8 (fix-release-transcribe task 6.2)
// ==========================================

type TestModelOrchestrator =
    Arc<Mutex<ModelOrchestrator<InjectableMockStore, MockSequenceDownloader>>>;

fn inject_valid_model_orchestrator(
    model_orchestrator: &TestModelOrchestrator,
    model_path: PathBuf,
) {
    *model_orchestrator.lock().expect("lock model orchestrator") = ModelOrchestrator::new(
        InjectableMockStore::injected(model_path),
        MockSequenceDownloader {
            progress_series: vec![],
        },
    );
}

struct WiredTranscribePipeline {
    orchestrator: Arc<Mutex<dyn TranscribeOrchestrator>>,
    lifecycle: TranscribeLifecycleHook,
    recorded_blocks: Arc<Mutex<Vec<TranscriptBlock>>>,
    pcm_bus: PcmChunkBus,
    model_orchestrator: TestModelOrchestrator,
}

fn build_deferred_transcribe_pipeline(segments: Vec<WhisperSegment>) -> WiredTranscribePipeline {
    let model_orchestrator = Arc::new(Mutex::new(ModelOrchestrator::new(
        InjectableMockStore::deferred(),
        MockSequenceDownloader {
            progress_series: vec![],
        },
    )));

    let (block_bus, recorded_blocks) = recording_block_bus();

    let (pcm_prod, pcm_cons) = rtrb::RingBuffer::<f32>::new(BATCH_WINDOW_SAMPLES * 3);
    let pcm_ingest = Arc::new(PcmIngestConsumer::new(pcm_prod));
    let pcm_bus = PcmChunkBus::new();
    pcm_bus.register(pcm_ingest);

    let block_emitter = Arc::new(BlockEmitter::new(Arc::clone(&block_bus)));
    let engine = MockSegmentEngine {
        segments,
        inference_called: Arc::new(AtomicBool::new(false)),
    };
    let mut worker =
        TranscribeWorker::with_engine(block_emitter as Arc<dyn TranscriptSegmentSink>, engine);
    worker.attach_pcm_consumer(pcm_cons);
    let worker_port = MockEngineWorkerPort { inner: worker };

    let orchestrator = Arc::new(Mutex::new(DefaultTranscribeOrchestrator::new(
        worker_port,
        NoopWhisperContextPort,
        Arc::clone(&model_orchestrator),
        std::time::Duration::from_millis(500),
    )));

    let lifecycle = TranscribeLifecycleHook::new(
        Arc::clone(&orchestrator) as Arc<Mutex<dyn TranscribeOrchestrator>>,
        Arc::new(RecordingTranscribeEventEmitter::default()),
    );

    WiredTranscribePipeline {
        orchestrator,
        lifecycle,
        recorded_blocks,
        pcm_bus,
        model_orchestrator,
    }
}

fn report_model_load_error(
    orchestrator: &Arc<Mutex<dyn TranscribeOrchestrator>>,
    emitter: &RecordingTranscribeEventEmitter,
    err: TranscribeError,
) {
    let mut orch = orchestrator.lock().expect("lock orchestrator");
    orch.fail_model_loading();
    let _ = emitter.emit_error(&err);
    let _ = emitter.emit_phase_changed(orch.phase());
}

/// Integration Test 6: deferred inject → ready → transcribing → block delivery (req 1.1, 1.2, 2.3, 5.3)
#[test]
fn integration_6_deferred_inject_ready_transcribing_block_delivery() {
    let model_path = create_temp_model_file();
    let pipeline = build_deferred_transcribe_pipeline(vec![WhisperSegment {
        text: "inject後の転写".to_string(),
        start_ms: 2000,
        end_ms: 4500,
    }]);

    assert_eq!(
        pipeline.orchestrator.lock().unwrap().phase(),
        TranscribePhase::Idle
    );

    inject_valid_model_orchestrator(&pipeline.model_orchestrator, model_path.clone());

    pipeline
        .orchestrator
        .lock()
        .unwrap()
        .ensure_model()
        .expect("model inject should allow ensure_model");
    assert_eq!(
        pipeline.orchestrator.lock().unwrap().phase(),
        TranscribePhase::Ready
    );

    pipeline
        .lifecycle
        .on_capture_phase_changed(CapturePhase::Capturing);
    assert_eq!(
        pipeline.orchestrator.lock().unwrap().phase(),
        TranscribePhase::Transcribing
    );

    publish_speech_pcm(&pipeline.pcm_bus, CHUNKS_PER_BATCH, 0);
    wait_for_blocks(&pipeline.recorded_blocks);

    let blocks = pipeline.recorded_blocks.lock().unwrap();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].text, "inject後の転写");
    assert_eq!(blocks[0].sequence, 1);

    pipeline
        .lifecycle
        .on_capture_phase_changed(CapturePhase::Stopping);

    let _ = std::fs::remove_file(model_path);
}

/// Integration Test 7: model fetch failure surfaces error phase and user-facing copy (req 4.1, 4.2)
#[test]
fn integration_7_model_fetch_failure_surfaces_user_facing_error() {
    let model_orchestrator = Arc::new(Mutex::new(ModelOrchestrator::new(
        DeferredPlaceholderStore,
        MockFailingDownloader,
    )));

    let orchestrator: Arc<Mutex<dyn TranscribeOrchestrator>> =
        Arc::new(Mutex::new(DefaultTranscribeOrchestrator::new(
            NoopTranscribeWorkerPort,
            NoopWhisperContextPort,
            Arc::clone(&model_orchestrator),
            std::time::Duration::from_millis(500),
        )));

    let emitter = Arc::new(RecordingTranscribeEventEmitter::default());
    let _hook = TranscribeLifecycleHook::new(
        Arc::clone(&orchestrator) as Arc<Mutex<dyn TranscribeOrchestrator>>,
        emitter.clone(),
    );

    orchestrator
        .lock()
        .unwrap()
        .begin_model_loading()
        .expect("begin loading");

    let acquire_err = {
        let model = model_orchestrator.lock().expect("lock model orchestrator");
        model
            .ensure_model(|_| {})
            .expect_err("download should fail")
    };

    report_model_load_error(&orchestrator, emitter.as_ref(), acquire_err.clone());

    assert_eq!(orchestrator.lock().unwrap().phase(), TranscribePhase::Error);
    assert!(matches!(
        acquire_err,
        TranscribeError::ModelDownloadFailed { .. }
    ));

    let user_errors = emitter.user_errors.lock().unwrap().clone();
    assert_eq!(user_errors.len(), 1);
    assert_eq!(
        user_errors[0].code,
        TranscribeErrorCode::ModelDownloadFailed
    );
    assert!(!user_errors[0].message_ja.is_empty());
    assert!(user_errors[0].action_ja_is_present());
    assert!(
        !user_errors[0].message_ja.contains("network unreachable"),
        "message_ja must not leak internal detail"
    );

    let phases = emitter.phases.lock().unwrap().clone();
    assert_eq!(phases.last().copied(), Some(TranscribePhase::Error));
}

/// Integration Test 8: capture stop returns ready and preserves emitted blocks (req 1.3, 1.4, 3.3)
#[test]
fn integration_8_capture_stop_returns_ready_and_preserves_blocks() {
    let model_path = create_temp_model_file();
    let pipeline = build_deferred_transcribe_pipeline(vec![WhisperSegment {
        text: "停止後も保持".to_string(),
        start_ms: 500,
        end_ms: 3000,
    }]);

    inject_valid_model_orchestrator(&pipeline.model_orchestrator, model_path.clone());

    pipeline
        .orchestrator
        .lock()
        .unwrap()
        .ensure_model()
        .expect("ensure model");
    pipeline
        .lifecycle
        .on_capture_phase_changed(CapturePhase::Capturing);
    publish_speech_pcm(&pipeline.pcm_bus, CHUNKS_PER_BATCH, 0);
    wait_for_blocks(&pipeline.recorded_blocks);

    let blocks_before_stop = pipeline.recorded_blocks.lock().unwrap().clone();
    assert_eq!(blocks_before_stop.len(), 1);
    assert_eq!(blocks_before_stop[0].text, "停止後も保持");

    pipeline
        .lifecycle
        .on_capture_phase_changed(CapturePhase::Stopping);

    assert_eq!(
        pipeline.orchestrator.lock().unwrap().phase(),
        TranscribePhase::Ready
    );

    let blocks_after_stop = pipeline.recorded_blocks.lock().unwrap().clone();
    assert_eq!(blocks_after_stop, blocks_before_stop);

    let _ = std::fs::remove_file(model_path);
}

// ==========================================
// Batch pipeline integration (task 5.1)
// ==========================================

/// 合成 PCM → PcmChunkBus → バッチ worker → モック adapter で sequence 欠番なし (req 2.4, 3.2)
#[test]
fn integration_batch_pipeline_emits_contiguous_sequences() {
    let mut fixture = setup_batch_pipeline(CountingBatchEngine {
        cycle: Arc::new(AtomicUsize::new(0)),
    });

    publish_speech_pcm(&fixture.pcm_bus, CHUNKS_PER_BATCH * 2, 0);
    wait_for_block_count(&fixture.recorded_blocks, 2);

    let blocks = stop_batch_worker_and_take_blocks(
        &mut fixture.worker,
        &fixture.recorded_blocks,
        "stop batch worker",
    );
    assert_eq!(blocks.len(), 2);
    assert_contiguous_sequences(&blocks);
    assert_eq!(blocks[0].text, "batch-0");
    assert_eq!(blocks[1].text, "batch-1");
}

/// 推論失敗注入後もバックログで次サイクルが実行される (req 2.4)
#[test]
fn integration_batch_pipeline_continues_after_inference_failure() {
    let attempts = Arc::new(AtomicU64::new(0));
    let mut fixture = setup_batch_pipeline(FailOnceBatchEngine {
        attempts: Arc::clone(&attempts),
    });

    publish_speech_pcm(
        &fixture.pcm_bus,
        CHUNKS_PER_BATCH * 2 + CHUNKS_PER_BATCH / 2,
        0,
    );
    wait_for_block_count(&fixture.recorded_blocks, 1);

    let blocks = stop_batch_worker_and_take_blocks(
        &mut fixture.worker,
        &fixture.recorded_blocks,
        "stop batch worker",
    );
    assert!(
        !blocks.is_empty(),
        "recovery cycle must emit at least one block"
    );
    assert_eq!(blocks[0].text, "recovered-batch");
    assert_contiguous_sequences(&blocks);
    assert!(
        attempts.load(Ordering::SeqCst) >= 2,
        "failed cycle must be retried on backlog"
    );
}

/// 停止 flush で残 PCM が最終バッチとして処理される (req 2.3, 6.3)
#[test]
fn integration_batch_pipeline_stop_flush_processes_remaining_pcm() {
    let mut fixture = setup_batch_pipeline(MockSegmentEngine {
        segments: vec![WhisperSegment {
            text: "flush-batch".to_string(),
            start_ms: 250,
            end_ms: 750,
        }],
        inference_called: Arc::new(AtomicBool::new(false)),
    });

    publish_speech_pcm(&fixture.pcm_bus, CHUNKS_PER_BATCH / 6, 0);
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert!(
        fixture.recorded_blocks.lock().unwrap().is_empty(),
        "partial buffer must not infer before stop flush"
    );

    let blocks = stop_batch_worker_and_take_blocks(
        &mut fixture.worker,
        &fixture.recorded_blocks,
        "stop batch worker",
    );
    assert_eq!(
        blocks.len(),
        1,
        "stop flush must emit a block for remaining PCM"
    );
    assert_eq!(blocks[0].text, "flush-batch");
    assert_eq!(blocks[0].sequence, 1);
    assert_eq!(blocks[0].start_timestamp_ms, 250);
}
