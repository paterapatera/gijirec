//! End-to-end capture processing: rtrb → resampler → mixer → chunk emitter → bus.

use gijirec_presentation::application::capture::chunk_emitter::ChunkEmitter;
use gijirec_presentation::application::capture::mixer::{AudioMixer, DefaultAudioMixer};
use gijirec_presentation::infrastructure::audio::MicSampleConsumer;
use gijirec_presentation::infrastructure::audio::resampler::{
    MonoResamplerPipeline, ResampledSampleConsumer, TARGET_SAMPLE_RATE_HZ,
};
use gijirec_presentation::tauri::pcm_bus::PcmChunkBus;
use gijirec_presentation::tauri::observability;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::capture_ports::{CaptureStreamHandles, SystemSampleConsumer};

const DRAIN_BUFFER_SAMPLES: usize = 1_024;
const LOOP_SLEEP: Duration = Duration::from_millis(5);
const RT_METRICS_LOG_INTERVAL: u64 = 20;

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
    mixer: DefaultAudioMixer,
    chunk_emitter: ChunkEmitter,
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
            mut mixer,
            mut chunk_emitter,
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
                push_drained_samples(
                    &scratch[..mic_count],
                    mic_rate_hz,
                    &mut mic_stream,
                    true,
                    &mut mixer,
                );
            }

            let (sys_count, sys_us) = drain_with_timing(system.as_mut(), &mut scratch);
            if sys_count > 0 {
                rt_drain_max_us = rt_drain_max_us.max(sys_us);
                push_drained_samples(
                    &scratch[..sys_count],
                    system_rate_hz,
                    &mut system_stream,
                    false,
                    &mut mixer,
                );
            }

            mixed.clear();
            let _ = mixer.drain_mixed(&mut mixed);
            if !mixed.is_empty() {
                chunk_emitter.push_mixed(&mixed);
                for chunk in chunk_emitter.emit_ready() {
                    pcm_bus.publish(chunk);
                }
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

        chunk_emitter.stop();
    }
}

fn push_drained_samples(
    drained: &[f32],
    source_rate_hz: u32,
    stream: &mut ResampledStream,
    is_mic: bool,
    mixer: &mut DefaultAudioMixer,
) {
    if drained.is_empty() {
        return;
    }

    let samples_16k: Vec<f32> = if source_rate_hz == TARGET_SAMPLE_RATE_HZ {
        drained.to_vec()
    } else {
        stream.ensure_resampler(source_rate_hz);
        let Some(pipeline) = stream.resampler.as_ref() else {
            return;
        };
        let Some(resampled) = stream.resampled.as_mut() else {
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

    let timeline = stream.timeline_samples;
    if is_mic {
        mixer.push_mic(&samples_16k, timeline);
    } else {
        mixer.push_system(&samples_16k, timeline);
    }
    stream.timeline_samples += samples_16k.len() as u64;
}

/// Handle to the dedicated capture processing thread.
pub(crate) struct CaptureProcessingHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl CaptureProcessingHandle {
    pub(crate) fn spawn(
        mic: Box<dyn F32SampleSource>,
        system: Box<dyn F32SampleSource>,
        mic_rate_hz: u32,
        system_rate_hz: u32,
        mixer: DefaultAudioMixer,
        chunk_emitter: ChunkEmitter,
        pcm_bus: Arc<PcmChunkBus>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let ctx = ProcessingContext {
            stop: Arc::clone(&stop),
            mixer,
            chunk_emitter,
            pcm_bus,
            mic,
            system,
            mic_rate_hz,
            system_rate_hz,
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
pub(crate) struct CapturePipelineState {
    pub mixer: Mutex<DefaultAudioMixer>,
    pub chunk_emitter: Mutex<ChunkEmitter>,
    pub pcm_bus: Arc<PcmChunkBus>,
    pub streams: CaptureStreamHandles,
    processing: Mutex<Option<CaptureProcessingHandle>>,
}

impl CapturePipelineState {
    pub(crate) fn new(streams: CaptureStreamHandles) -> Self {
        Self {
            mixer: Mutex::new(DefaultAudioMixer::new()),
            chunk_emitter: Mutex::new(ChunkEmitter::new()),
            pcm_bus: Arc::new(PcmChunkBus::new()),
            streams,
            processing: Mutex::new(None),
        }
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
        let mut emitter_guard = self.chunk_emitter.lock().expect("lock");
        let mixer = std::mem::replace(&mut *mixer_guard, DefaultAudioMixer::new());
        let chunk_emitter = std::mem::replace(&mut *emitter_guard, ChunkEmitter::new());
        let pcm_bus = Arc::clone(&self.pcm_bus);

        *guard = Some(CaptureProcessingHandle::spawn(
            Box::new(mic),
            Box::new(system),
            mic_rate_hz,
            system_rate_hz,
            mixer,
            chunk_emitter,
            pcm_bus,
        ));
        Ok(())
    }

    pub(crate) fn stop_processing(&self) {
        if let Some(handle) = self.processing.lock().expect("lock").take() {
            handle.stop();
        }
    }

    /// Returns whether the dedicated processing thread is still running.
    #[cfg(test)]
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
pub(crate) enum ProcessingStartError {
    MicConsumerMissing,
    SystemConsumerMissing,
    MicRateMissing,
    SystemRateMissing,
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_presentation::domain::audio::pcm_chunk::{
        CHUNK_FRAME_COUNT, PcmChunk, PcmChunkConsumer, PcmConsumerError, SAMPLE_RATE_HZ,
    };
    use rtrb::RingBuffer;
    use std::time::Instant;

    struct RingConsumer(rtrb::Consumer<f32>);

    impl F32SampleSource for RingConsumer {
        fn drain_into(&mut self, out: &mut [f32]) -> usize {
            let mut count = 0;
            for slot in out.iter_mut() {
                match self.0.pop() {
                    Ok(sample) => {
                        *slot = sample;
                        count += 1;
                    }
                    Err(_) => break,
                }
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

    #[test]
    fn pipeline_smoke_emits_100ms_chunks_with_monotonic_sequence() {
        let rate_hz = SAMPLE_RATE_HZ;
        let (mut producers, mic, system) = spawn_synthetic_sources(rate_hz);
        let bus = Arc::new(PcmChunkBus::new());
        let recorder = Arc::new(RecordingChunkConsumer::new());
        bus.register(Arc::clone(&recorder) as Arc<dyn PcmChunkConsumer>);

        let handle = CaptureProcessingHandle::spawn(
            mic,
            system,
            rate_hz,
            rate_hz,
            DefaultAudioMixer::new(),
            ChunkEmitter::new(),
            Arc::clone(&bus),
        );

        let pump = thread::spawn(move || {
            let frame_batch = (rate_hz / 10) as usize;
            for tick in 0..60 {
                for i in 0..frame_batch {
                    let sample = 0.2 * ((tick * frame_batch + i) as f32 * 0.01).sin();
                    let _ = producers.mic.push(sample);
                    let _ = producers.system.push(sample * 0.5);
                }
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
}
