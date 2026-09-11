use std::sync::{Arc, Mutex};

use gijirec_presentation::application::capture::orchestrator::CaptureOrchestrator;
use gijirec_presentation::application::capture_audio_controls::{
    CaptureAudioControlsApplyPort, CapturePhasePort, IngestSourcePort,
};
use gijirec_presentation::application::device_selection::{
    CaptureSelectionPort, DeviceEnumeratorPort, DeviceSelectionError,
};
use gijirec_presentation::domain::audio::{
    AudioDeviceList, CaptureError, CapturePhase, DeviceSelection,
};
use gijirec_presentation::infrastructure::audio::device_enumerator::{
    AudioDeviceEnumerator, EnumeratorError,
};

use crate::capture_processing::{CapturePipelineState, CaptureProcessingGate};
use gijirec_presentation::transcribe::PcmIngestConsumer;

pub(crate) struct OrchestratorCapturePhasePort {
    pub(crate) orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
}

impl CapturePhasePort for OrchestratorCapturePhasePort {
    fn capture_phase(&self) -> CapturePhase {
        self.orchestrator
            .lock()
            .expect("lock capture orchestrator")
            .phase()
    }
}

pub(crate) struct CaptureAudioControlsApplyPortAdapter {
    pub(crate) mic_gate: CaptureProcessingGate,
    pub(crate) pcm_ingest: Arc<PcmIngestConsumer>,
}

impl CaptureAudioControlsApplyPort for CaptureAudioControlsApplyPortAdapter {
    fn set_mic_ingest_enabled(&self, enabled: bool) {
        self.mic_gate.set_mic_ingest_enabled(enabled);
    }

    fn set_ingest_gain_multiplier(&self, gain: f32) {
        self.pcm_ingest.set_ingest_gain_multiplier(gain);
    }
}

pub(crate) struct ComposeIngestSourcePort {
    pub(crate) orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
    pub(crate) pipeline: Arc<CapturePipelineState>,
}

impl IngestSourcePort for ComposeIngestSourcePort {
    fn has_ingestable_audio_source(&self, mic_enabled: bool) -> bool {
        if mic_enabled {
            return true;
        }
        self.orchestrator.lock().expect("lock").phase() == CapturePhase::Capturing
            && self.pipeline.processing_is_active()
    }
}

pub(crate) struct DeviceEnumeratorPortAdapter {
    inner: AudioDeviceEnumerator,
}

impl DeviceEnumeratorPortAdapter {
    pub(crate) fn new(inner: AudioDeviceEnumerator) -> Self {
        Self { inner }
    }
}

impl DeviceEnumeratorPort for DeviceEnumeratorPortAdapter {
    fn list_devices(&self) -> Result<AudioDeviceList, DeviceSelectionError> {
        self.inner
            .list_devices()
            .map_err(|err: EnumeratorError| DeviceSelectionError::internal(err.to_string()))
    }
}

pub(crate) struct CaptureSelectionPortAdapter {
    orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
    pipeline: Arc<CapturePipelineState>,
}

impl CaptureSelectionPortAdapter {
    pub(crate) fn new(
        orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
        pipeline: Arc<CapturePipelineState>,
    ) -> Self {
        Self {
            orchestrator,
            pipeline,
        }
    }
}

impl CaptureSelectionPort for CaptureSelectionPortAdapter {
    fn capture_phase(&self) -> gijirec_presentation::domain::audio::CapturePhase {
        self.orchestrator.lock().expect("lock").phase()
    }

    fn restart_with_selection(&mut self, selection: &DeviceSelection) -> Result<(), CaptureError> {
        self.pipeline.stop_processing_for_recapture();
        let result = self
            .orchestrator
            .lock()
            .expect("lock")
            .restart_with_selection(selection);
        if result.is_ok() {
            let _ = self.pipeline.start_processing();
        }
        result
    }
}
