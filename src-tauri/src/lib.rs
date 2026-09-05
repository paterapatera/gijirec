pub mod capture_observability;
mod capture_ports;
mod capture_processing;
mod compose;

use capture_observability::TracingCaptureObservability;
use compose::build_capture_stack;
use gijirec_presentation::tauri::lifecycle::{
    CaptureLifecycleState, CaptureProcessingHook, OsCapturePlatformSupport,
    OsUnsupportedPlatformNotifier, attach_capture_lifecycle, handle_capture_run_event,
};
use gijirec_presentation::tauri::observability::{init_session_id, set_observability};
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("gijirec_capture=info,info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    set_observability(Box::new(TracingCaptureObservability));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();
    init_session_id();
    let composed = build_capture_stack();
    let lifecycle = CaptureLifecycleState::new(
        composed.orchestrator,
        Arc::new(OsCapturePlatformSupport),
        Arc::new(OsUnsupportedPlatformNotifier),
    );
    let pipeline = Arc::new(composed.pipeline);
    lifecycle.set_processing_hook(Arc::clone(&pipeline) as Arc<dyn CaptureProcessingHook>);

    let app = attach_capture_lifecycle(tauri::Builder::default(), lifecycle)
        .manage(pipeline)
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        handle_capture_run_event(app_handle, &event);
    });
}
