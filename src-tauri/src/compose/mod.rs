//! Composition root: orchestrator, pipeline hold, transcribe wiring, and lifecycle helpers.

mod audio_controls;
mod late_bound;
mod model_stack;
mod port_adapters;
pub(crate) mod wiring;

pub(crate) use audio_controls::{
    CachingIngestLevelEventEmitter, CaptureAudioControlsProcessingHook,
};
pub(crate) use late_bound::{LateBoundCaptureAudioControlsEvents, LateBoundDeviceSelectionEvents};
pub(crate) use model_stack::{SharedModelOrchestrator, inject_model_stack_shared};
#[allow(unused_imports)]
pub(crate) use wiring::PCM_RTRB_CAPACITY_SAMPLES;
pub(crate) use wiring::compose_with_ports_and_model_orchestrator;

use gijirec_presentation::application::capture::orchestrator::CaptureOrchestrator;
use gijirec_presentation::application::capture_audio_controls::CaptureAudioControlsService;
use gijirec_presentation::application::device_selection::DeviceSelectionService;
use gijirec_presentation::application::transcribe::orchestrator::TranscribeOrchestrator;
use gijirec_presentation::tauri::capture_audio_controls::IngestLevelSnapshotCache;
use gijirec_presentation::transcribe::{TranscribeLifecycleHook, TranscriptBlockBus};
use std::sync::{Arc, Mutex};

use crate::capture_ports::CaptureStreamHandles;
use crate::capture_processing::CapturePipelineState;

use model_stack::deferred_model_orchestrator;

/// Fully composed capture and transcribe stack ready for Tauri lifecycle injection.
pub(crate) struct ComposedCapture {
    pub orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
    pub device_selection: Arc<dyn DeviceSelectionService>,
    pub device_selection_events: Arc<LateBoundDeviceSelectionEvents>,
    pub pipeline: Arc<CapturePipelineState>,
    pub capture_audio_controls: Arc<dyn CaptureAudioControlsService>,
    pub capture_audio_controls_events: Arc<LateBoundCaptureAudioControlsEvents>,
    pub capture_audio_controls_hook: Arc<CaptureAudioControlsProcessingHook>,
    pub ingest_level_events: Arc<CachingIngestLevelEventEmitter>,
    pub ingest_level_cache: IngestLevelSnapshotCache,
    pub transcribe_lifecycle: Arc<TranscribeLifecycleHook>,
    pub transcribe_bus: Arc<TranscriptBlockBus>,
    pub transcribe_orchestrator: Arc<Mutex<dyn TranscribeOrchestrator>>,
    pub model_orchestrator: SharedModelOrchestrator,
}

/// Builds the production capture and transcribe stack with platform and whisper adapters.
/// Model acquisition uses a deferred placeholder until setup calls [`inject_model_stack`].
pub(crate) fn build_capture_stack() -> ComposedCapture {
    let (streams, mic, system) = CaptureStreamHandles::new_pair();
    compose_with_ports_and_model_orchestrator(mic, system, streams, deferred_model_orchestrator())
}

/// Extra parameters for initializing the transcribe stack in debug harnesses.
#[cfg(debug_assertions)]
struct TranscribeComposeConfig {
    app_data_dir: std::path::PathBuf,
}

/// Builds capture + transcribe stack from injectable ports and data directories.
#[cfg(debug_assertions)]
fn compose_with_ports_and_transcribe<M, S>(
    mic: M,
    system: S,
    streams: CaptureStreamHandles,
    config: TranscribeComposeConfig,
) -> ComposedCapture
/* jscpd:ignore-start */
where
    M: gijirec_presentation::application::capture::orchestrator::MicCapturePort + 'static,
    S: gijirec_presentation::application::capture::orchestrator::SystemAudioCapturePort + 'static,
    /* jscpd:ignore-end */
{
    use model_stack::{build_model_orchestrator, wrap_model_orchestrator};

    let model_orchestrator = wrap_model_orchestrator(build_model_orchestrator(config.app_data_dir));
    compose_with_ports_and_model_orchestrator(mic, system, streams, model_orchestrator)
}

/// Builds capture stack with default dummy transcribe stack for testing.
#[cfg(debug_assertions)]
pub(crate) fn compose_with_ports<M, S>(
    mic: M,
    system: S,
    streams: CaptureStreamHandles,
) -> ComposedCapture
where
    M: gijirec_presentation::application::capture::orchestrator::MicCapturePort + 'static,
    S: gijirec_presentation::application::capture::orchestrator::SystemAudioCapturePort + 'static,
{
    compose_with_ports_and_transcribe(
        mic,
        system,
        streams,
        TranscribeComposeConfig {
            app_data_dir: std::env::temp_dir().join("gijirec_test"),
        },
    )
}

#[cfg(test)]
mod tests;
