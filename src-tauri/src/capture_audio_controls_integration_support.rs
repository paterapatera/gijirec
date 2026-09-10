//! Composed-stack harness for capture-audio-controls integration tests (task 10.1).

#[cfg(debug_assertions)]
use std::sync::atomic::AtomicUsize;
#[cfg(debug_assertions)]
use std::sync::{Arc, Mutex};

#[cfg(debug_assertions)]
use crate::capture_ports::{SyntheticMicPort, SyntheticSystemPort};
#[cfg(debug_assertions)]
use crate::compose::compose_with_ports;
#[cfg(debug_assertions)]
use crate::test_support::new_stream_handles;
#[cfg(debug_assertions)]
use gijirec_presentation::application::capture_audio_controls::CaptureAudioControlsEvents;
#[cfg(debug_assertions)]
use gijirec_presentation::domain::audio::CapturePhase;
#[cfg(debug_assertions)]
use gijirec_presentation::domain::audio::DeviceSelection;
#[cfg(debug_assertions)]
use gijirec_presentation::tauri::capture_audio_controls::{
    CaptureAudioControlsPatchRequest, CaptureAudioControlsStateResponse,
    RecordingCaptureAudioControlsEventEmitter, get_capture_audio_controls_impl,
    set_capture_audio_controls_impl,
};
#[cfg(debug_assertions)]
use gijirec_presentation::tauri::device_selection::set_device_selection_impl;
#[cfg(debug_assertions)]
use gijirec_presentation::tauri::lifecycle::CaptureProcessingHook;
#[cfg(debug_assertions)]
use gijirec_presentation::transcribe::{IngestLevelEmitter, PcmIngestConsumer};

/// Fully composed capture stack with synthetic ports for integration tests.
#[cfg(debug_assertions)]
pub struct CaptureAudioControlsIntegrationStack {
    composed: crate::compose::ComposedCapture,
    events: Arc<RecordingCaptureAudioControlsEventEmitter>,
    mic_prod: Arc<Mutex<Option<rtrb::Producer<f32>>>>,
    sys_prod: Arc<Mutex<Option<rtrb::Producer<f32>>>>,
}

#[cfg(debug_assertions)]
impl Default for CaptureAudioControlsIntegrationStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(debug_assertions)]
impl CaptureAudioControlsIntegrationStack {
    pub fn new() -> Self {
        let mic_opened = Arc::new(Mutex::new(false));
        let sys_opened = Arc::new(Mutex::new(false));
        let mic_prod = Arc::new(Mutex::new(None));
        let sys_prod = Arc::new(Mutex::new(None));

        let streams = new_stream_handles();
        let mic_port = SyntheticMicPort::new_instrumented(
            streams.clone(),
            Arc::clone(&mic_opened),
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicUsize::new(0)),
            Arc::clone(&mic_prod),
        );
        let sys_port = SyntheticSystemPort::new_instrumented(
            streams.clone(),
            Arc::clone(&sys_opened),
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicUsize::new(0)),
            Arc::clone(&sys_prod),
        );

        let composed = compose_with_ports(mic_port, sys_port, streams);
        let events = Arc::new(RecordingCaptureAudioControlsEventEmitter::new());
        composed
            .capture_audio_controls_events
            .set_emitter(Arc::clone(&events) as Arc<dyn CaptureAudioControlsEvents>);

        Self {
            composed,
            events,
            mic_prod,
            sys_prod,
        }
    }

    pub fn pcm_ingest(&self) -> &Arc<PcmIngestConsumer> {
        &self.composed.pcm_ingest
    }

    pub fn ingest_level_emitter(&self) -> &Arc<IngestLevelEmitter> {
        &self.composed.ingest_level_emitter
    }

    pub fn pcm_bus(&self) -> &Arc<gijirec_presentation::tauri::pcm_bus::PcmChunkBus> {
        &self.composed.pipeline.pcm_bus
    }

    pub fn events(&self) -> &RecordingCaptureAudioControlsEventEmitter {
        self.events.as_ref()
    }

    pub fn set_controls(
        &self,
        patch: CaptureAudioControlsPatchRequest,
    ) -> CaptureAudioControlsStateResponse {
        set_capture_audio_controls_impl(
            self.composed.capture_audio_controls.as_ref(),
            &self.composed.ingest_level_cache,
            patch,
        )
        .expect("set_capture_audio_controls")
    }

    pub fn get_controls(&self) -> CaptureAudioControlsStateResponse {
        get_capture_audio_controls_impl(
            self.composed.capture_audio_controls.as_ref(),
            &self.composed.ingest_level_cache,
        )
    }

    pub fn start_capturing(&self) {
        let mut orch = self
            .composed
            .orchestrator
            .lock()
            .expect("lock orchestrator");
        orch.start_with_selection(&DeviceSelection::default())
            .expect("start capture");
        assert_eq!(orch.phase(), CapturePhase::Capturing);
        drop(orch);

        self.composed
            .capture_audio_controls_hook
            .on_capture_started();
        self.composed
            .pipeline
            .start_processing()
            .expect("start processing");
        assert!(self.composed.pipeline.processing_is_active());
    }

    pub fn stop_processing_keep_capturing(&self) {
        self.composed.pipeline.stop_processing_for_recapture();
        assert!(!self.composed.pipeline.processing_is_active());
        assert_eq!(
            self.composed
                .orchestrator
                .lock()
                .expect("lock orchestrator")
                .phase(),
            CapturePhase::Capturing
        );
    }

    pub fn simulate_device_recapture(&self, selection: DeviceSelection) {
        set_device_selection_impl(self.composed.device_selection.as_ref(), selection)
            .expect("device recapture");
        assert_eq!(
            self.composed
                .orchestrator
                .lock()
                .expect("lock orchestrator")
                .phase(),
            CapturePhase::Capturing
        );
        assert!(self.composed.pipeline.processing_is_active());
    }

    pub fn pump_samples(&self, samples: usize) {
        let mut mic = self.mic_prod.lock().expect("lock mic producer");
        let mut sys = self.sys_prod.lock().expect("lock sys producer");
        let mic = mic.as_mut().expect("mic producer must be open");
        let sys = sys.as_mut().expect("sys producer must be open");
        for i in 0..samples {
            let sample = 0.2 * ((i as f32) * 0.01).sin();
            let _ = mic.push(sample);
            let _ = sys.push(sample * 0.5);
        }
    }

    pub fn on_capture_stopping(&self) {
        self.composed
            .capture_audio_controls_hook
            .on_capture_stopping();
    }

    pub fn processing_is_active(&self) -> bool {
        self.composed.pipeline.processing_is_active()
    }

    pub fn mic_ingest_enabled_in_store(&self) -> bool {
        self.get_controls().controls.mic_ingest_enabled
    }

    pub fn alternate_device_selection(&self) -> DeviceSelection {
        let list = self
            .composed
            .device_selection
            .list_devices()
            .expect("list devices");
        let current = self.composed.device_selection.get_selection();
        if let Some(mic) = list.inputs.iter().find(|device| {
            current
                .microphone_id()
                .map(|id| id.as_str() != device.id().as_str())
                .unwrap_or(true)
        }) {
            return DeviceSelection::new(Some(mic.id().clone()), current.speaker_id().cloned());
        }
        DeviceSelection::new(None, current.speaker_id().cloned())
    }
}
