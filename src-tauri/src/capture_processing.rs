//! End-to-end capture processing: rtrb → resampler → mixer → chunk emitter → bus.

use gijirec_presentation::application::capture::chunk_emitter::ChunkEmitter;
use gijirec_presentation::application::capture::mixer::{AudioMixer, DefaultAudioMixer};
use gijirec_presentation::infrastructure::audio::MicSampleConsumer;
use gijirec_presentation::infrastructure::audio::resampler::{
    MonoResamplerPipeline, ResampledSampleConsumer, TARGET_SAMPLE_RATE_HZ,
};
use gijirec_presentation::tauri::observability;
use gijirec_presentation::tauri::pcm_bus::PcmChunkBus;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::capture_ports::{CaptureStreamHandles, SystemSampleConsumer};

const DRAIN_BUFFER_SAMPLES: usize = 1_024;
const LOOP_SLEEP: Duration = Duration::from_millis(5);
const RT_METRICS_LOG_INTERVAL: u64 = 20;

/// Pushes synthetic sine mic/sys samples into paired rtrb producers (test harness).
#[cfg(any(test, debug_assertions))]
pub(crate) fn pump_rtrb_mic_sys_producers(
    mic: &mut rtrb::Producer<f32>,
    sys: &mut rtrb::Producer<f32>,
    samples: usize,
) {
    for i in 0..samples {
        let sample = 0.2 * ((i as f32) * 0.01).sin();
        let _ = mic.push(sample);
        let _ = sys.push(sample * 0.5);
    }
}

/// Shared mic ingest gate readable from the capture processing thread.
#[derive(Debug, Clone)]
pub(crate) struct CaptureProcessingGate {
    mic_ingest_enabled: Arc<AtomicBool>,
}

impl CaptureProcessingGate {
    pub(crate) fn new() -> Self {
        Self {
            mic_ingest_enabled: Arc::new(AtomicBool::new(true)),
        }
    }

    pub(crate) fn mic_ingest_enabled(&self) -> bool {
        self.mic_ingest_enabled.load(Ordering::SeqCst)
    }

    pub(crate) fn set_mic_ingest_enabled(&self, enabled: bool) {
        self.mic_ingest_enabled.store(enabled, Ordering::SeqCst);
    }
}

fn drain_with_timing(source: &mut dyn F32SampleSource, scratch: &mut [f32]) -> (usize, u64) {
    let start = Instant::now();
    let count = source.drain_into(scratch);
    (count, start.elapsed().as_micros() as u64)
}

/// Drains mono f32 samples from an RT ring buffer consumer.
pub(crate) trait F32SampleSource: Send {
    fn drain_into(&mut self, out: &mut [f32]) -> usize;
}

impl F32SampleSource for MicSampleConsumer {
    fn drain_into(&mut self, out: &mut [f32]) -> usize {
        MicSampleConsumer::drain_into(self, out)
    }
}

impl F32SampleSource for SystemSampleConsumer {
    fn drain_into(&mut self, out: &mut [f32]) -> usize {
        match self {
            #[cfg(target_os = "windows")]
            Self::Loopback(consumer) => consumer.drain_into(out),
            #[cfg(target_os = "macos")]
            Self::Sck(consumer) => consumer.drain_into(out),
            #[cfg(target_os = "linux")]
            Self::Unavailable => 0,
        }
    }
}

struct ResampledStream {
    resampler: Option<MonoResamplerPipeline>,
    resampled: Option<ResampledSampleConsumer>,
    timeline_samples: u64,
}

impl ResampledStream {
    fn new() -> Self {
        Self {
            resampler: None,
            resampled: None,
            timeline_samples: 0,
        }
    }

    fn ensure_resampler(&mut self, source_rate_hz: u32) {
        if self.resampler.is_none() && source_rate_hz != TARGET_SAMPLE_RATE_HZ {
            let (pipeline, consumer) = MonoResamplerPipeline::spawn(source_rate_hz, 1);
            self.resampler = Some(pipeline);
            self.resampled = Some(consumer);
        }
    }
}

struct ProcessingContext {
    stop: Arc<AtomicBool>,
    mic_gate: CaptureProcessingGate,
    mixer: DefaultAudioMixer,
    chunk_emitter: Arc<Mutex<ChunkEmitter>>,
    pcm_bus: Arc<PcmChunkBus>,
    mic: Box<dyn F32SampleSource>,
    system: Box<dyn F32SampleSource>,
    mic_rate_hz: u32,
    system_rate_hz: u32,
    mic_stream: ResampledStream,
    system_stream: ResampledStream,
}

impl ProcessingContext {
    fn run(self) {
        let ProcessingContext {
            stop,
            mic_gate,
            mut mixer,
            chunk_emitter,
            pcm_bus,
            mut mic,
            mut system,
            mic_rate_hz,
            system_rate_hz,
            mut mic_stream,
            mut system_stream,
        } = self;

        let mut scratch = [0.0_f32; DRAIN_BUFFER_SAMPLES];
        let mut mixed = Vec::with_capacity(DRAIN_BUFFER_SAMPLES);
        let mut loop_count: u64 = 0;
        let mut rt_drain_max_us: u64 = 0;
        let mut last_logged_rt_max_us: u64 = 0;

        while !stop.load(Ordering::SeqCst) {
            let (mic_count, mic_us) = drain_with_timing(mic.as_mut(), &mut scratch);
            if mic_count > 0 {
                rt_drain_max_us = rt_drain_max_us.max(mic_us);
            }
            if mic_count > 0 && mic_gate.mic_ingest_enabled() {
                push_drained_samples(
                    &scratch[..mic_count],
                    mic_rate_hz,
                    StreamRoute {
                        stream: &mut mic_stream,
                        is_mic: true,
                    },
                    &mut mixer,
                );
            }

            let (sys_count, sys_us) = drain_with_timing(system.as_mut(), &mut scratch);
            if sys_count > 0 {
                rt_drain_max_us = rt_drain_max_us.max(sys_us);
                push_drained_samples(
                    &scratch[..sys_count],
                    system_rate_hz,
                    StreamRoute {
                        stream: &mut system_stream,
                        is_mic: false,
                    },
                    &mut mixer,
                );
            }

            mixed.clear();
            let _ = mixer.drain_mixed(&mut mixed);
            if !mixed.is_empty() {
                let mut emitter = chunk_emitter.lock().expect("lock");
                emitter.push_mixed(&mixed);
                publish_ready_chunks(&mut emitter, &pcm_bus);
            }

            loop_count += 1;
            if loop_count.is_multiple_of(RT_METRICS_LOG_INTERVAL)
                && rt_drain_max_us > last_logged_rt_max_us
            {
                observability::log_rt_callback_max_us(rt_drain_max_us);
                last_logged_rt_max_us = rt_drain_max_us;
            }

            thread::sleep(LOOP_SLEEP);
        }
    }
}

fn publish_ready_chunks(chunk_emitter: &mut ChunkEmitter, pcm_bus: &PcmChunkBus) {
    for chunk in chunk_emitter.emit_ready() {
        pcm_bus.publish(chunk);
    }
}

struct StreamRoute<'a> {
    stream: &'a mut ResampledStream,
    is_mic: bool,
}

fn push_drained_samples(
    drained: &[f32],
    source_rate_hz: u32,
    route: StreamRoute<'_>,
    mixer: &mut DefaultAudioMixer,
) {
    if drained.is_empty() {
        return;
    }

    let samples_16k: Vec<f32> = if source_rate_hz == TARGET_SAMPLE_RATE_HZ {
        drained.to_vec()
    } else {
        route.stream.ensure_resampler(source_rate_hz);
        let Some(pipeline) = route.stream.resampler.as_ref() else {
            return;
        };
        let Some(resampled) = route.stream.resampled.as_mut() else {
            return;
        };
        pipeline.input().push_interleaved(drained);
        let mut out = [0.0_f32; DRAIN_BUFFER_SAMPLES];
        let produced = resampled.drain_into(&mut out);
        if produced == 0 {
            return;
        }
        out[..produced].to_vec()
    };

    if samples_16k.is_empty() {
        return;
    }

    let timeline = route.stream.timeline_samples;
    if route.is_mic {
        mixer.push_mic(&samples_16k, timeline);
    } else {
        mixer.push_system(&samples_16k, timeline);
    }
    route.stream.timeline_samples += samples_16k.len() as u64;
}

pub(crate) struct ProcessingSpawnParams {
    pub mic: Box<dyn F32SampleSource>,
    pub system: Box<dyn F32SampleSource>,
    pub mic_rate_hz: u32,
    pub system_rate_hz: u32,
    pub mixer: DefaultAudioMixer,
    pub chunk_emitter: Arc<Mutex<ChunkEmitter>>,
    pub pcm_bus: Arc<PcmChunkBus>,
    pub mic_gate: CaptureProcessingGate,
}

/// Handle to the dedicated capture processing thread.
pub(crate) struct CaptureProcessingHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl CaptureProcessingHandle {
    pub(crate) fn spawn(params: ProcessingSpawnParams) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let ctx = ProcessingContext {
            stop: Arc::clone(&stop),
            mic_gate: params.mic_gate,
            mixer: params.mixer,
            chunk_emitter: params.chunk_emitter,
            pcm_bus: params.pcm_bus,
            mic: params.mic,
            system: params.system,
            mic_rate_hz: params.mic_rate_hz,
            system_rate_hz: params.system_rate_hz,
            mic_stream: ResampledStream::new(),
            system_stream: ResampledStream::new(),
        };
        let thread = thread::Builder::new()
            .name("capture-processing".into())
            .spawn(move || ctx.run())
            .expect("spawn capture processing thread");
        Self {
            stop,
            thread: Some(thread),
        }
    }

    pub(crate) fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

/// Owns pipeline state and coordinates processing thread lifecycle.
#[allow(unreachable_pub)] // `src-tauri/tests/` integration harnesses consume via `test_support`
/// Capture pipeline state; `pub` for `tests/` integration harnesses (`test_support`).
pub struct CapturePipelineState {
    pub mixer: Mutex<DefaultAudioMixer>,
    pub chunk_emitter: Arc<Mutex<ChunkEmitter>>,
    pub pcm_bus: Arc<PcmChunkBus>,
    pub streams: CaptureStreamHandles,
    mic_gate: CaptureProcessingGate,
    processing: Mutex<Option<CaptureProcessingHandle>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessingStopMode {
    Recapture,
    Final,
}

impl CapturePipelineState {
    pub(crate) fn new(streams: CaptureStreamHandles) -> Self {
        Self {
            mixer: Mutex::new(DefaultAudioMixer::new()),
            chunk_emitter: Arc::new(Mutex::new(ChunkEmitter::new())),
            pcm_bus: Arc::new(PcmChunkBus::new()),
            streams,
            mic_gate: CaptureProcessingGate::new(),
            processing: Mutex::new(None),
        }
    }

    pub(crate) fn mic_gate(&self) -> &CaptureProcessingGate {
        &self.mic_gate
    }

    pub(crate) fn set_stream_disconnect_handler(
        &self,
        handler: crate::capture_ports::StreamDisconnectHandler,
    ) {
        self.streams.set_stream_disconnect_handler(handler);
    }

    pub(crate) fn start_processing(&self) -> Result<(), ProcessingStartError> {
        let mut guard = self.processing.lock().expect("lock");
        if guard.is_some() {
            return Ok(());
        }

        let mic = self
            .streams
            .take_mic_consumer()
            .ok_or(ProcessingStartError::MicConsumerMissing)?;
        let system = self
            .streams
            .take_system_consumer()
            .ok_or(ProcessingStartError::SystemConsumerMissing)?;
        let mic_rate_hz = self
            .streams
            .mic_sample_rate_hz()
            .ok_or(ProcessingStartError::MicRateMissing)?;
        let system_rate_hz = self
            .streams
            .system_sample_rate_hz()
            .ok_or(ProcessingStartError::SystemRateMissing)?;

        let mut mixer_guard = self.mixer.lock().expect("lock");
        let mixer = std::mem::replace(&mut *mixer_guard, DefaultAudioMixer::new());
        let chunk_emitter = Arc::clone(&self.chunk_emitter);
        let pcm_bus = Arc::clone(&self.pcm_bus);

        *guard = Some(CaptureProcessingHandle::spawn(ProcessingSpawnParams {
            mic: Box::new(mic),
            system: Box::new(system),
            mic_rate_hz,
            system_rate_hz,
            mixer,
            chunk_emitter,
            pcm_bus,
            mic_gate: self.mic_gate.clone(),
        }));
        Ok(())
    }

    fn stop_processing_with_mode(&self, mode: ProcessingStopMode) {
        if let Some(handle) = self.processing.lock().expect("lock").take() {
            handle.stop();
            let mut emitter = self.chunk_emitter.lock().expect("lock");
            match mode {
                ProcessingStopMode::Recapture => emitter.discard_partial_buffer(),
                ProcessingStopMode::Final => emitter.stop(),
            }
            *self.mixer.lock().expect("lock") = DefaultAudioMixer::new();
        }
    }

    pub(crate) fn stop_processing_for_recapture(&self) {
        self.stop_processing_with_mode(ProcessingStopMode::Recapture);
    }

    pub(crate) fn stop_processing(&self) {
        self.stop_processing_with_mode(ProcessingStopMode::Final);
    }

    /// Returns whether the dedicated processing thread is still running.
    pub(crate) fn processing_is_active(&self) -> bool {
        self.processing.lock().expect("lock").is_some()
    }
}

impl gijirec_presentation::tauri::lifecycle::CaptureProcessingHook for CapturePipelineState {
    fn on_capture_started(&self) {
        let _ = self.start_processing();
    }

    fn on_capture_stopping(&self) {
        self.stop_processing();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum ProcessingStartError {
    MicConsumerMissing,
    SystemConsumerMissing,
    MicRateMissing,
    SystemRateMissing,
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_presentation::application::capture::orchestrator::{
        CaptureOrchestrator, DefaultCaptureOrchestrator,
    };
    use gijirec_presentation::domain::audio::pcm_chunk::{
        CHUNK_FRAME_COUNT, PcmChunk, PcmChunkConsumer, PcmConsumerError, SAMPLE_RATE_HZ,
    };
    use gijirec_presentation::tauri::lifecycle::CaptureProcessingHook;
    use rtrb::RingBuffer;
    use std::time::Instant;

    struct RingConsumer(rtrb::Consumer<f32>);

    impl F32SampleSource for RingConsumer {
        fn drain_into(&mut self, out: &mut [f32]) -> usize {
            let mut count = 0;
            for slot in out.iter_mut() {
                let sample = match self.0.pop() {
                    Ok(s) => s,
                    Err(_) => break,
                };
                *slot = sample;
                count += 1;
            }
            count
        }
    }

    struct RecordingChunkConsumer {
        chunks: Mutex<Vec<PcmChunk>>,
    }

    impl RecordingChunkConsumer {
        fn new() -> Self {
            Self {
                chunks: Mutex::new(Vec::new()),
            }
        }

        fn chunks(&self) -> Vec<PcmChunk> {
            self.chunks.lock().expect("lock").clone()
        }
    }

    impl PcmChunkConsumer for RecordingChunkConsumer {
        fn on_pcm_chunk(&self, chunk: PcmChunk) -> Result<(), PcmConsumerError> {
            self.chunks.lock().expect("lock").push(chunk);
            Ok(())
        }
    }

    struct ProducerHandle {
        mic: rtrb::Producer<f32>,
        system: rtrb::Producer<f32>,
    }

    fn spawn_synthetic_sources(
        rate_hz: u32,
    ) -> (
        ProducerHandle,
        Box<dyn F32SampleSource>,
        Box<dyn F32SampleSource>,
    ) {
        let (mic_prod, mic_cons) = RingBuffer::<f32>::new(rate_hz as usize * 2);
        let (sys_prod, sys_cons) = RingBuffer::<f32>::new(rate_hz as usize * 2);
        (
            ProducerHandle {
                mic: mic_prod,
                system: sys_prod,
            },
            Box::new(RingConsumer(mic_cons)),
            Box::new(RingConsumer(sys_cons)),
        )
    }

    fn push_synthetic_tick(producers: &mut ProducerHandle, tick: usize, frame_batch: usize) {
        for i in 0..frame_batch {
            let sample = 0.2 * ((tick * frame_batch + i) as f32 * 0.01).sin();
            let _ = producers.mic.push(sample);
            let _ = producers.system.push(sample * 0.5);
        }
    }

    fn frame_batch_10ms(rate_hz: u32) -> usize {
        (rate_hz / 100) as usize
    }

    struct RecordingPipeline {
        handle: CaptureProcessingHandle,
        recorder: Arc<RecordingChunkConsumer>,
        producers: ProducerHandle,
        frame_batch: usize,
    }

    fn spawn_recording_pipeline(rate_hz: u32, gate: CaptureProcessingGate) -> RecordingPipeline {
        let (producers, mic, system) = spawn_synthetic_sources(rate_hz);
        let bus = Arc::new(PcmChunkBus::new());
        let recorder = Arc::new(RecordingChunkConsumer::new());
        bus.register(Arc::clone(&recorder) as Arc<dyn PcmChunkConsumer>);

        let handle = CaptureProcessingHandle::spawn(ProcessingSpawnParams {
            mic,
            system,
            mic_rate_hz: rate_hz,
            system_rate_hz: rate_hz,
            mixer: DefaultAudioMixer::new(),
            chunk_emitter: Arc::new(Mutex::new(ChunkEmitter::new())),
            pcm_bus: Arc::clone(&bus),
            mic_gate: gate,
        });

        RecordingPipeline {
            handle,
            recorder,
            producers,
            frame_batch: frame_batch_10ms(rate_hz),
        }
    }

    #[derive(Clone, Copy)]
    struct MicSystemPump {
        mic_amplitude: f32,
        system_amplitude: f32,
    }

    const DUAL_SOURCE_PUMP: MicSystemPump = MicSystemPump {
        mic_amplitude: 0.6,
        system_amplitude: 0.25,
    };

    const SYSTEM_ONLY_PUMP: MicSystemPump = MicSystemPump {
        mic_amplitude: 0.0,
        system_amplitude: 0.25,
    };

    fn push_mic_system_sine(
        producers: &mut ProducerHandle,
        tick: usize,
        frame_batch: usize,
        pump: &MicSystemPump,
    ) {
        for i in 0..frame_batch {
            let n = (tick * frame_batch + i) as f32;
            let _ = producers.mic.push(pump.mic_amplitude * (n * 0.13).sin());
            let _ = producers
                .system
                .push(pump.system_amplitude * (n * 0.17).sin());
        }
    }

    fn chunk_rms(chunk: &PcmChunk) -> f64 {
        let sum_sq = chunk
            .samples()
            .iter()
            .map(|s| {
                let v = *s as f64 / 32_768.0;
                v * v
            })
            .sum::<f64>();
        (sum_sq / chunk.samples().len() as f64).sqrt()
    }

    fn chunks_have_audible_rms(chunks: &[PcmChunk], threshold: f64) -> bool {
        chunks.iter().any(|chunk| chunk_rms(chunk) > threshold)
    }

    #[test]
    // 回帰: system ソースが無配信のまま経過した後（＝mic より 50 ms 以上遅れて配信開始）に
    // 届いた音声がミックスへ乗る

    fn system_audio_arriving_after_idle_gap_reaches_mixed_output() {
        let mut pipeline = spawn_recording_pipeline(SAMPLE_RATE_HZ, CaptureProcessingGate::new());
        let frame_batch = pipeline.frame_batch;
        // 前半 600 ms: mic は無音（ゲート内）、system は無配信
        for _ in 0..60 {
            for _ in 0..frame_batch {
                let _ = pipeline.producers.mic.push(0.0);
            }
            thread::sleep(Duration::from_millis(10));
        }
        // 後半 600 ms: system が再生開始（0.3 のサイン波）、mic は引き続き無音
        for tick in 0..60_usize {
            for i in 0..frame_batch {
                let _ = pipeline.producers.mic.push(0.0);
                let n = (tick * frame_batch + i) as f32;
                let _ = pipeline.producers.system.push(0.3 * (n * 0.17).sin());
            }
            thread::sleep(Duration::from_millis(10));
        }
        thread::sleep(Duration::from_millis(100));
        pipeline.handle.stop();

        let chunks = pipeline.recorder.chunks();
        assert!(
            chunks.len() >= 8,
            "expected several chunks, got {}",
            chunks.len()
        );
        let last = &chunks[chunks.len() - 2..];
        assert!(
            chunks_have_audible_rms(last, 0.03),
            "system audio that started after an idle gap must appear in the mixed output"
        );
    }

    #[test]
    fn pipeline_smoke_emits_100ms_chunks_with_monotonic_sequence() {
        let rate_hz = SAMPLE_RATE_HZ;
        let pipeline = spawn_recording_pipeline(rate_hz, CaptureProcessingGate::new());
        let recorder = Arc::clone(&pipeline.recorder);
        let mut producers = pipeline.producers;
        let handle = pipeline.handle;

        let pump = thread::spawn(move || {
            let frame_batch = (rate_hz / 10) as usize;
            for tick in 0..60 {
                push_synthetic_tick(&mut producers, tick, frame_batch);
                thread::sleep(Duration::from_millis(10));
            }
        });

        let deadline = Instant::now() + Duration::from_millis(300);
        while recorder.chunks().is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }

        let first_deadline = Instant::now() + Duration::from_millis(500);
        while recorder.chunks().len() < 2 && Instant::now() < first_deadline {
            thread::sleep(Duration::from_millis(5));
        }

        pump.join().expect("pump");
        handle.stop();

        let chunks = recorder.chunks();
        assert!(
            !chunks.is_empty(),
            "expected at least one PcmChunk within 300 ms"
        );
        assert_eq!(chunks[0].frame_count(), CHUNK_FRAME_COUNT);
        assert_eq!(chunks[0].samples().len(), CHUNK_FRAME_COUNT as usize);
        assert_eq!(chunks[0].sequence(), 0);

        if chunks.len() >= 2 {
            assert_eq!(chunks[1].sequence(), 1);
            let delta = chunks[1]
                .timestamp_ms()
                .saturating_sub(chunks[0].timestamp_ms());
            assert!(
                (80..=120).contains(&delta),
                "expected ~100 ms between chunks, got {delta} ms"
            );
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)] // Integration-style test: full recapture + emitter sequence assertions.
    fn recapture_processing_preserves_chunk_emitter_sequence() {
        use crate::capture_ports::{CaptureStreamHandles, SyntheticMicPort, SyntheticSystemPort};
        use std::sync::atomic::{AtomicUsize, Ordering};

        let mic_opened = Arc::new(Mutex::new(false));
        let sys_opened = Arc::new(Mutex::new(false));
        let mic_open_count = Arc::new(AtomicUsize::new(0));
        let mic_close_count = Arc::new(AtomicUsize::new(0));
        let sys_open_count = Arc::new(AtomicUsize::new(0));
        let sys_close_count = Arc::new(AtomicUsize::new(0));
        let mic_prod = Arc::new(Mutex::new(None));
        let sys_prod = Arc::new(Mutex::new(None));

        let (streams, _mic_port, _sys_port) = CaptureStreamHandles::new_pair();
        let mic_port = SyntheticMicPort::new_instrumented(
            streams.clone(),
            Arc::clone(&mic_opened),
            Arc::clone(&mic_open_count),
            Arc::clone(&mic_close_count),
            Arc::clone(&mic_prod),
        );
        let sys_port = SyntheticSystemPort::new_instrumented(
            streams.clone(),
            Arc::clone(&sys_opened),
            Arc::clone(&sys_open_count),
            Arc::clone(&sys_close_count),
            Arc::clone(&sys_prod),
        );

        let pipeline = CapturePipelineState::new(streams);
        let recorder = Arc::new(RecordingChunkConsumer::new());
        pipeline
            .pcm_bus
            .register(Arc::clone(&recorder) as Arc<dyn PcmChunkConsumer>);

        let orchestrator = Arc::new(Mutex::new(DefaultCaptureOrchestrator::new(
            mic_port, sys_port,
        )));
        orchestrator
            .lock()
            .expect("lock")
            .start_with_selection(&gijirec_presentation::domain::audio::DeviceSelection::default())
            .expect("start");
        pipeline.on_capture_started();

        let frames_per_chunk = CHUNK_FRAME_COUNT as usize;
        pump_producers(&mic_prod, &sys_prod, frames_per_chunk * 4);

        let deadline = Instant::now() + Duration::from_secs(2);
        while recorder.chunks().len() < 3 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        let before = recorder
            .chunks()
            .iter()
            .map(PcmChunk::sequence)
            .collect::<Vec<_>>();
        assert!(
            before.len() >= 3,
            "expected chunks before recapture: {before:?}"
        );
        let last_before = *before.last().expect("last");
        let seq_before_recapture = pipeline.chunk_emitter.lock().expect("lock").next_sequence();
        assert!(seq_before_recapture >= 3);

        pipeline.stop_processing_for_recapture();
        orchestrator
            .lock()
            .expect("lock")
            .restart_with_selection(
                &gijirec_presentation::domain::audio::DeviceSelection::default(),
            )
            .expect("orchestrator restart");
        pipeline.start_processing().expect("restart processing");

        assert!(mic_close_count.load(Ordering::SeqCst) >= 1);
        assert!(mic_open_count.load(Ordering::SeqCst) >= 2);

        pump_producers(&mic_prod, &sys_prod, frames_per_chunk * 4);
        let after_deadline = Instant::now() + Duration::from_secs(2);
        while recorder.chunks().len() < before.len() + 3 && Instant::now() < after_deadline {
            thread::sleep(Duration::from_millis(5));
        }

        let all: Vec<u64> = recorder.chunks().iter().map(PcmChunk::sequence).collect();
        for window in all.windows(2) {
            assert_eq!(
                window[1],
                window[0] + 1,
                "sequence must stay monotonic across recapture: {all:?}"
            );
        }
        let after_change = &all[before.len()..];
        assert!(!after_change.is_empty());
        assert_eq!(
            after_change[0],
            last_before + 1,
            "ChunkEmitter must not reset on recapture: last_before={last_before} after={after_change:?}"
        );
        assert_eq!(
            pipeline.chunk_emitter.lock().expect("lock").next_sequence(),
            after_change.last().expect("last after") + 1
        );

        pipeline.on_capture_stopping();
    }

    fn pump_producers(
        mic_prod: &Arc<Mutex<Option<rtrb::Producer<f32>>>>,
        sys_prod: &Arc<Mutex<Option<rtrb::Producer<f32>>>>,
        samples: usize,
    ) {
        let mut mic = mic_prod.lock().expect("lock");
        let mut sys = sys_prod.lock().expect("lock");
        let mic = mic.as_mut().expect("mic producer must be installed");
        let sys = sys.as_mut().expect("sys producer must be installed");
        pump_rtrb_mic_sys_producers(mic, sys, samples);
    }

    fn max_audible_chunk_rms(recorder: &RecordingChunkConsumer) -> f64 {
        recorder
            .chunks()
            .iter()
            .map(chunk_rms)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0)
    }

    fn run_gate_scenario(
        gate: &CaptureProcessingGate,
        pump: MicSystemPump,
        pump_ticks: usize,
    ) -> f64 {
        let mut pipeline = spawn_recording_pipeline(SAMPLE_RATE_HZ, gate.clone());
        for tick in 0..pump_ticks {
            push_mic_system_sine(&mut pipeline.producers, tick, pipeline.frame_batch, &pump);
            thread::sleep(Duration::from_millis(10));
        }
        thread::sleep(Duration::from_millis(150));
        pipeline.handle.stop();

        max_audible_chunk_rms(&pipeline.recorder)
    }

    #[test]
    fn mic_ingest_gate_off_supplies_system_only() {
        let gate_off = CaptureProcessingGate::new();
        gate_off.set_mic_ingest_enabled(false);

        let gate_on = CaptureProcessingGate::new();
        let rms_gate_off = run_gate_scenario(&gate_off, DUAL_SOURCE_PUMP, 80);
        let rms_system_only = run_gate_scenario(&gate_on, SYSTEM_ONLY_PUMP, 80);

        assert!(
            rms_gate_off > 0.02,
            "system audio must reach mixed output when mic gate is off: rms={rms_gate_off}"
        );
        assert!(
            (rms_gate_off - rms_system_only).abs() < 0.03,
            "gate off must match system-only mix (off={rms_gate_off}, system_only={rms_system_only})"
        );
    }

    #[test]
    fn mic_ingest_gate_on_includes_mic_in_mixer() {
        let gate_on = CaptureProcessingGate::new();

        let rms_dual = run_gate_scenario(&gate_on, DUAL_SOURCE_PUMP, 80);
        let rms_system_only = run_gate_scenario(&gate_on, SYSTEM_ONLY_PUMP, 80);

        assert!(
            rms_dual > rms_system_only + 0.02,
            "gate on must mix mic with system (dual={rms_dual}, system_only={rms_system_only})"
        );
    }

    #[test]
    fn mic_ingest_gate_toggle_reflects_on_next_chunk() {
        let gate = CaptureProcessingGate::new();
        let mut pipeline = spawn_recording_pipeline(SAMPLE_RATE_HZ, gate.clone());

        for tick in 0..40 {
            push_mic_system_sine(
                &mut pipeline.producers,
                tick,
                pipeline.frame_batch,
                &DUAL_SOURCE_PUMP,
            );
            thread::sleep(Duration::from_millis(10));
        }

        let rms_before_toggle = max_audible_chunk_rms(&pipeline.recorder);
        gate.set_mic_ingest_enabled(false);

        for tick in 40..80 {
            push_mic_system_sine(
                &mut pipeline.producers,
                tick,
                pipeline.frame_batch,
                &DUAL_SOURCE_PUMP,
            );
            thread::sleep(Duration::from_millis(10));
        }
        thread::sleep(Duration::from_millis(150));
        pipeline.handle.stop();

        let chunks = pipeline.recorder.chunks();
        assert!(
            chunks.len() >= 4,
            "expected several chunks, got {}",
            chunks.len()
        );
        let late = &chunks[chunks.len() - 2..];
        let rms_after_toggle = late
            .iter()
            .map(chunk_rms)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);

        assert!(
            rms_before_toggle > rms_after_toggle + 0.02,
            "toggle off must reduce mix level on subsequent chunks (before={rms_before_toggle}, after={rms_after_toggle})"
        );
        assert!(
            rms_after_toggle > 0.02,
            "system audio must remain after mic gate off: rms={rms_after_toggle}"
        );
    }
}
