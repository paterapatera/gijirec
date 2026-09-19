//! Capture session service wiring (compose root).

use std::sync::{Arc, Mutex};

use gijirec_presentation::application::capture::orchestrator::CaptureOrchestrator;
use gijirec_presentation::application::capture_session::{
    CaptureSessionObservability, CaptureSessionPlatform, CaptureSessionProcessingHook,
    CaptureSessionService, CaptureSessionServiceApi,
};
use gijirec_presentation::application::device_selection::DeviceSelectionService;
use gijirec_presentation::tauri::lifecycle::{CaptureProcessingHook, is_capture_supported_os};

use super::late_bound::LateBoundCaptureSessionEvents;

/// OS probe for session start gating (mirrors lifecycle platform support).
pub(crate) struct OsCaptureSessionPlatform;

impl CaptureSessionPlatform for OsCaptureSessionPlatform {
    fn is_capture_supported(&self) -> bool {
        is_capture_supported_os()
    }
}

struct ProcessingHookAdapter(Arc<dyn CaptureProcessingHook>);

impl CaptureSessionProcessingHook for ProcessingHookAdapter {
    fn on_capture_started(&self) {
        self.0.on_capture_started();
    }

    fn on_capture_stopping(&self) {
        self.0.on_capture_stopping();
    }
}

/// Invokes the same processing hooks as capture lifecycle on session start/stop.
pub(crate) struct ChainedCaptureSessionProcessingHook {
    hooks: Vec<Arc<dyn CaptureSessionProcessingHook>>,
}

impl ChainedCaptureSessionProcessingHook {
    pub(crate) fn from_capture_processing_hooks(
        hooks: Vec<Arc<dyn CaptureProcessingHook>>,
    ) -> Self {
        Self {
            hooks: hooks
                .into_iter()
                .map(|hook| {
                    Arc::new(ProcessingHookAdapter(hook)) as Arc<dyn CaptureSessionProcessingHook>
                })
                .collect(),
        }
    }
}

impl CaptureSessionProcessingHook for ChainedCaptureSessionProcessingHook {
    fn on_capture_started(&self) {
        for hook in &self.hooks {
            hook.on_capture_started();
        }
    }

    fn on_capture_stopping(&self) {
        for hook in &self.hooks {
            hook.on_capture_stopping();
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn init_capture_session(
    orchestrator: Arc<Mutex<dyn CaptureOrchestrator>>,
    device_selection: Arc<dyn DeviceSelectionService>,
    processing: Arc<ChainedCaptureSessionProcessingHook>,
    events: Arc<LateBoundCaptureSessionEvents>,
    observability: Arc<dyn CaptureSessionObservability>,
) -> Arc<dyn CaptureSessionServiceApi> {
    let session_events: Arc<
        dyn gijirec_presentation::application::capture_session::CaptureSessionEvents,
    > = events;
    let service: Arc<dyn CaptureSessionServiceApi> = Arc::new(CaptureSessionService::new(
        orchestrator,
        device_selection,
        Arc::new(OsCaptureSessionPlatform),
        processing,
        session_events,
        Arc::new(gijirec_presentation::application::capture_session::SystemCaptureSessionClock),
        observability,
    ));
    service
}
