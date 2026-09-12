//! Shared transcribe presentation test fixtures.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use gijirec_application::transcribe::{
    ModelDownloadProgress, ModelDownloaderPort, ModelStorePort, TranscribeWorkerPort,
};
use gijirec_domain::audio::pcm_chunk::{CHUNK_FRAME_COUNT, PcmChunk};
use gijirec_domain::transcribe::{
    TranscribeError, TranscribePhase, TranscriptBlock, TranscriptBlockConsumer,
    TranscriptConsumerError, TranscriptSegmentSink, UserFacingTranscribeError, WhisperModelVariant,
};
use gijirec_infrastructure::transcribe::{
    ModelPathLoadable, SegmentEngine, TranscribeWorker, WhisperSegment,
};

use super::event_emitter::{TranscribeEmitError, TranscribeEventEmitter};
use super::pcm_ingest_consumer::PcmIngestConsumer;
use super::stall_watchdog::StallClock;
use super::transcript_block_bus::TranscriptBlockBus;
use gijirec_application::transcribe::block_emitter::BlockEmitter;

pub use gijirec_application::transcribe::noop_ports::{
    NoopTranscribeWorkerPort, NoopWhisperContextPort,
};

/// One 30 s batch window at 16 kHz (matches production worker constant).
pub const BATCH_WINDOW_SAMPLES: usize = 480_000;

pub const CHUNKS_PER_BATCH: usize = BATCH_WINDOW_SAMPLES / CHUNK_FRAME_COUNT as usize;

use gijirec_infrastructure::noop_model_path_loadable;

fn record_transcribe_phase(
    phases: &Arc<Mutex<Vec<TranscribePhase>>>,
    phase: TranscribePhase,
) -> Result<(), TranscribeEmitError> {
    phases.lock().expect("lock phases").push(phase);
    Ok(())
}

pub fn mock_stall_clock() -> (StallClock, Arc<AtomicU64>) {
    let time = Arc::new(AtomicU64::new(0));
    let clock: StallClock = {
        let time = Arc::clone(&time);
        Arc::new(move || time.load(Ordering::SeqCst))
    };
    (clock, time)
}

pub fn stall_clock_only() -> StallClock {
    mock_stall_clock().0
}

pub fn clone_store_model_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

pub fn clone_store_model_path_for(path: &Path, _variant: WhisperModelVariant) -> PathBuf {
    clone_store_model_path(path)
}

#[derive(Default)]
pub struct RecordingBlockConsumer {
    pub blocks: Arc<Mutex<Vec<TranscriptBlock>>>,
}

impl TranscriptBlockConsumer for RecordingBlockConsumer {
    fn on_block_appended(&self, block: TranscriptBlock) -> Result<(), TranscriptConsumerError> {
        self.blocks.lock().expect("lock blocks").push(block);
        Ok(())
    }
}

/// Lightweight transcribe emitter for unit tests (stall watchdog, lifecycle hook).
pub type MockTranscribeEmitter = RecordingTranscribeEventEmitter;

#[derive(Default)]
pub struct RecordingTranscribeEventEmitter {
    pub phases: Arc<Mutex<Vec<TranscribePhase>>>,
    pub errors: Arc<Mutex<Vec<TranscribeError>>>,
    pub user_errors: Arc<Mutex<Vec<UserFacingTranscribeError>>>,
    pub progress: Arc<Mutex<Vec<ModelDownloadProgress>>>,
}

impl TranscribeEventEmitter for RecordingTranscribeEventEmitter {
    fn emit_phase_changed(&self, phase: TranscribePhase) -> Result<(), TranscribeEmitError> {
        record_transcribe_phase(&self.phases, phase)
    }

    fn emit_model_progress(
        &self,
        progress: &ModelDownloadProgress,
    ) -> Result<(), TranscribeEmitError> {
        self.progress
            .lock()
            .expect("lock progress")
            .push(progress.clone());
        Ok(())
    }

    fn emit_error(&self, error: &TranscribeError) -> Result<(), TranscribeEmitError> {
        self.errors.lock().expect("lock errors").push(error.clone());
        self.user_errors
            .lock()
            .expect("lock user errors")
            .push(error.to_user_facing());
        Ok(())
    }
}

pub fn recording_block_bus() -> (Arc<TranscriptBlockBus>, Arc<Mutex<Vec<TranscriptBlock>>>) {
    let block_consumer = Arc::new(RecordingBlockConsumer::default());
    let recorded_blocks = Arc::clone(&block_consumer.blocks);
    let block_bus = Arc::new(TranscriptBlockBus::new());
    block_bus.register(block_consumer);
    (block_bus, recorded_blocks)
}

pub struct MockStore {
    pub path: PathBuf,
}

impl ModelStorePort for MockStore {
    fn model_path(&self) -> PathBuf {
        clone_store_model_path(&self.path)
    }

    fn model_path_for(&self, variant: WhisperModelVariant) -> PathBuf {
        clone_store_model_path_for(&self.path, variant)
    }

    fn verify(&self, _expected: Option<&str>) -> Result<PathBuf, TranscribeError> {
        Ok(self.path.clone())
    }

    fn verify_variant(
        &self,
        _variant: WhisperModelVariant,
        _expected: Option<&str>,
    ) -> Result<PathBuf, TranscribeError> {
        Ok(self.path.clone())
    }

    fn file_exists(&self, _variant: WhisperModelVariant) -> bool {
        true
    }
}

pub struct MockSequenceDownloader {
    pub progress_series: Vec<ModelDownloadProgress>,
}

impl ModelDownloaderPort for MockSequenceDownloader {
    fn download(
        &self,
        _url: &str,
        _dest: &Path,
        on_progress: &mut dyn FnMut(ModelDownloadProgress),
    ) -> Result<(), TranscribeError> {
        for progress in &self.progress_series {
            on_progress(progress.clone());
        }
        Ok(())
    }
}

pub struct MockSegmentEngine {
    pub segments: Vec<WhisperSegment>,
    pub inference_called: Arc<AtomicBool>,
}

impl SegmentEngine for MockSegmentEngine {
    fn transcribe_pcm(&mut self, _pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError> {
        self.inference_called.store(true, Ordering::SeqCst);
        Ok(self.segments.clone())
    }

    fn is_loaded(&self) -> bool {
        true
    }
}

noop_model_path_loadable!(MockSegmentEngine);

pub struct CountingBatchEngine {
    pub cycle: Arc<AtomicUsize>,
}

impl SegmentEngine for CountingBatchEngine {
    fn transcribe_pcm(&mut self, _pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError> {
        let cycle = self.cycle.fetch_add(1, Ordering::SeqCst);
        Ok(vec![WhisperSegment {
            text: format!("batch-{cycle}"),
            start_ms: cycle as i64 * 1_000,
            end_ms: cycle as i64 * 1_000 + 500,
        }])
    }

    fn is_loaded(&self) -> bool {
        true
    }
}

noop_model_path_loadable!(CountingBatchEngine);

pub struct FailOnceBatchEngine {
    pub attempts: Arc<AtomicU64>,
}

impl SegmentEngine for FailOnceBatchEngine {
    fn transcribe_pcm(&mut self, pcm: &[f32]) -> Result<Vec<WhisperSegment>, TranscribeError> {
        if pcm.is_empty() {
            return Ok(Vec::new());
        }
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
        if attempt == 0 {
            return Err(TranscribeError::InferenceFailed {
                detail: "injected batch failure".to_string(),
            });
        }
        Ok(vec![WhisperSegment {
            text: "recovered-batch".to_string(),
            start_ms: 0,
            end_ms: 500,
        }])
    }

    fn is_loaded(&self) -> bool {
        true
    }
}

noop_model_path_loadable!(FailOnceBatchEngine);

pub struct BatchPipelineFixture<E: SegmentEngine + ModelPathLoadable + 'static> {
    pub pcm_bus: super::super::tauri::pcm_bus::PcmChunkBus,
    pub recorded_blocks: Arc<Mutex<Vec<TranscriptBlock>>>,
    pub worker: TranscribeWorker<E>,
}

pub fn setup_batch_pipeline<E: SegmentEngine + ModelPathLoadable + 'static>(
    engine: E,
) -> BatchPipelineFixture<E> {
    let pcm_bus = super::super::tauri::pcm_bus::PcmChunkBus::new();
    let (pcm_prod, pcm_cons) = rtrb::RingBuffer::<f32>::new(BATCH_WINDOW_SAMPLES * 3);
    pcm_bus.register(Arc::new(PcmIngestConsumer::new(pcm_prod)));

    let (block_bus, recorded_blocks) = recording_block_bus();
    let emitter = Arc::new(BlockEmitter::new(block_bus));

    let mut worker =
        TranscribeWorker::with_engine(emitter as Arc<dyn TranscriptSegmentSink>, engine);
    worker.attach_pcm_consumer(pcm_cons);
    worker.spawn().expect("spawn batch worker");

    BatchPipelineFixture {
        pcm_bus,
        recorded_blocks,
        worker,
    }
}

pub fn spawn_transcribe_worker<E: SegmentEngine + ModelPathLoadable + 'static>(
    emitter: Arc<BlockEmitter<TranscriptBlockBus>>,
    engine: E,
    pcm_consumer: rtrb::Consumer<f32>,
) -> TranscribeWorker<E> {
    let mut worker =
        TranscribeWorker::with_engine(emitter as Arc<dyn TranscriptSegmentSink>, engine);
    worker.attach_pcm_consumer(pcm_consumer);
    worker.spawn().expect("worker spawn");
    worker
}

pub fn publish_speech_pcm(
    pcm_bus: &super::super::tauri::pcm_bus::PcmChunkBus,
    chunk_count: usize,
    start_seq: u64,
) {
    for offset in 0..chunk_count {
        let seq = start_seq + offset as u64;
        let samples = vec![16384_i16; CHUNK_FRAME_COUNT as usize];
        let chunk = PcmChunk::new(seq, samples, seq * 100).expect("chunk");
        pcm_bus.publish(chunk);
    }
}

pub fn wait_for_block_count(recorded_blocks: &Arc<Mutex<Vec<TranscriptBlock>>>, expected: usize) {
    for _ in 0..100 {
        if recorded_blocks.lock().expect("lock blocks").len() >= expected {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "expected at least {expected} blocks, got {}",
        recorded_blocks.lock().expect("lock blocks").len()
    );
}

pub fn wait_for_blocks(recorded_blocks: &Arc<Mutex<Vec<TranscriptBlock>>>) {
    wait_for_block_count(recorded_blocks, 1);
}

pub fn assert_contiguous_sequences(blocks: &[TranscriptBlock]) {
    for (index, block) in blocks.iter().enumerate() {
        assert_eq!(
            block.sequence,
            (index + 1) as u64,
            "sequence must increase by one without gaps"
        );
    }
}

pub fn stop_batch_worker_and_take_blocks<E: SegmentEngine + ModelPathLoadable>(
    worker: &mut TranscribeWorker<E>,
    recorded_blocks: &Arc<Mutex<Vec<TranscriptBlock>>>,
    stop_label: &str,
) -> Vec<TranscriptBlock> {
    worker
        .stop_and_join(Duration::from_secs(2))
        .expect(stop_label);
    recorded_blocks.lock().expect("lock blocks").clone()
}

pub struct InjectableMockStore {
    state: Mutex<InjectableMockStoreState>,
}

struct InjectableMockStoreState {
    injected: bool,
    path: PathBuf,
}

macro_rules! impl_delegating_model_store_tail {
    () => {
        fn model_path_for(&self, variant: WhisperModelVariant) -> PathBuf {
            model_path_for_delegating(self, variant)
        }

        fn verify_variant(
            &self,
            variant: WhisperModelVariant,
            expected: Option<&str>,
        ) -> Result<PathBuf, TranscribeError> {
            verify_variant_delegating(self, variant, expected)
        }

        fn file_exists(&self, _variant: WhisperModelVariant) -> bool {
            false
        }
    };
}

impl InjectableMockStore {
    pub fn deferred() -> Self {
        Self::with_injected(false, PathBuf::from("/deferred/unavailable/model.bin"))
    }

    pub fn injected(path: PathBuf) -> Self {
        Self::with_injected(true, path)
    }

    fn with_injected(injected: bool, path: PathBuf) -> Self {
        Self {
            state: Mutex::new(InjectableMockStoreState { injected, path }),
        }
    }
}

impl ModelStorePort for InjectableMockStore {
    fn model_path(&self) -> PathBuf {
        self.state
            .lock()
            .expect("lock injectable store")
            .path
            .clone()
    }

    fn verify(&self, _expected: Option<&str>) -> Result<PathBuf, TranscribeError> {
        let state = self.state.lock().expect("lock injectable store");
        if !state.injected {
            return Err(TranscribeError::ModelNotFound {
                detail: "deferred until app_data_dir inject".to_string(),
            });
        }
        Ok(state.path.clone())
    }

    impl_delegating_model_store_tail!();
}

pub struct DeferredPlaceholderStore;

impl ModelStorePort for DeferredPlaceholderStore {
    fn model_path(&self) -> PathBuf {
        PathBuf::from("/deferred/unavailable/model.bin")
    }

    fn verify(&self, _expected: Option<&str>) -> Result<PathBuf, TranscribeError> {
        Err(TranscribeError::ModelNotFound {
            detail: "deferred until app_data_dir inject".to_string(),
        })
    }

    impl_delegating_model_store_tail!();
}

fn model_path_for_delegating(store: &dyn ModelStorePort, _variant: WhisperModelVariant) -> PathBuf {
    store.model_path()
}

fn verify_variant_delegating(
    store: &dyn ModelStorePort,
    _variant: WhisperModelVariant,
    expected: Option<&str>,
) -> Result<PathBuf, TranscribeError> {
    store.verify(expected)
}

pub struct MockFailingDownloader;

impl ModelDownloaderPort for MockFailingDownloader {
    fn download(
        &self,
        _url: &str,
        _dest: &Path,
        _on_progress: &mut dyn FnMut(ModelDownloadProgress),
    ) -> Result<(), TranscribeError> {
        Err(TranscribeError::ModelDownloadFailed {
            detail: "network unreachable".to_string(),
        })
    }
}

pub fn create_temp_model_file() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "gijirec-integration-model-{}.bin",
        std::process::id()
    ));
    std::fs::write(&path, b"mock-whisper-model").expect("write temp model file");
    path
}

pub struct MockWorkerPort {
    noop: NoopTranscribeWorkerPort,
    pub spawns: Arc<AtomicUsize>,
    pub stops: Arc<AtomicUsize>,
}

impl MockWorkerPort {
    pub fn new(spawns: Arc<AtomicUsize>, stops: Arc<AtomicUsize>) -> Self {
        Self {
            noop: NoopTranscribeWorkerPort,
            spawns,
            stops,
        }
    }
}

impl Clone for MockWorkerPort {
    fn clone(&self) -> Self {
        Self {
            noop: self.noop,
            spawns: Arc::clone(&self.spawns),
            stops: Arc::clone(&self.stops),
        }
    }
}

impl TranscribeWorkerPort for MockWorkerPort {
    fn prepare_model_path(&mut self, path: &Path) -> Result<(), TranscribeError> {
        self.noop.prepare_model_path(path)
    }

    fn spawn(&mut self) -> Result<(), TranscribeError> {
        self.spawns.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn stop_and_join(&mut self, _timeout: Duration) -> Result<(), TranscribeError> {
        self.stops.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

pub struct MockEngineWorkerPort<E: SegmentEngine + ModelPathLoadable + 'static> {
    pub inner: TranscribeWorker<E>,
}

impl<E: SegmentEngine + ModelPathLoadable + 'static> TranscribeWorkerPort
    for MockEngineWorkerPort<E>
{
    fn prepare_model_path(&mut self, path: &Path) -> Result<(), TranscribeError> {
        self.inner.prepare_model_path(path)
    }

    fn spawn(&mut self) -> Result<(), TranscribeError> {
        self.inner.spawn()
    }

    fn stop_and_join(&mut self, timeout: Duration) -> Result<(), TranscribeError> {
        self.inner.stop_and_join(timeout)
    }
}
