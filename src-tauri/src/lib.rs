pub mod capture_observability;
mod capture_ports;
mod capture_processing;
pub mod commands;
mod compose;
pub mod transcribe_observability;

use capture_observability::TracingCaptureObservability;
use commands::{get_capture_phase, get_transcribe_phase, get_transcribe_status};
use compose::{SharedModelOrchestrator, build_capture_stack};
use gijirec_presentation::application::transcribe::orchestrator::TranscribeOrchestrator;
use gijirec_presentation::application::transcribe::ports::{
    ModelDownloadProgress, ModelDownloadStatus,
};
use gijirec_presentation::domain::audio::CapturePhase;
use gijirec_presentation::domain::transcribe::{TranscribeError, TranscribePhase};
use gijirec_presentation::tauri::lifecycle::{
    CaptureLifecycleState, CaptureProcessingHook, OsCapturePlatformSupport,
    OsUnsupportedPlatformNotifier, attach_capture_lifecycle, handle_capture_run_event,
    run_capture_app_setup,
};
use gijirec_presentation::tauri::observability::{init_session_id, set_observability};
use gijirec_presentation::transcribe::TranscribeEventEmitter;
use gijirec_presentation::transcribe::TranscribeStatusCache;
use gijirec_presentation::transcribe::observability::set_transcribe_observability;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::Listener;
use tracing_subscriber::EnvFilter;
use transcribe_observability::TracingTranscribeObservability;

const MODEL_PROGRESS_EMIT_INTERVAL: Duration = Duration::from_millis(250);

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("gijirec_capture=info,gijirec_transcribe=info,info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    set_observability(Box::new(TracingCaptureObservability));
    set_transcribe_observability(Box::new(TracingTranscribeObservability));
}

struct ModelProgressThrottle {
    last_emit: Mutex<Option<Instant>>,
}

impl ModelProgressThrottle {
    fn new() -> Self {
        Self {
            last_emit: Mutex::new(None),
        }
    }

    fn should_emit(&self, force: bool) -> bool {
        if force {
            return true;
        }
        let mut last = self.last_emit.lock().expect("lock progress throttle");
        let now = Instant::now();
        let emit = last
            .map(|previous| now.duration_since(previous) >= MODEL_PROGRESS_EMIT_INTERVAL)
            .unwrap_or(true);
        if emit {
            *last = Some(now);
        }
        emit
    }
}

struct ModelLoadReporter {
    cache: Arc<TranscribeStatusCache>,
    emitter: Arc<dyn TranscribeEventEmitter>,
    throttle: ModelProgressThrottle,
}

impl ModelLoadReporter {
    fn new(cache: Arc<TranscribeStatusCache>, emitter: Arc<dyn TranscribeEventEmitter>) -> Self {
        Self {
            cache,
            emitter,
            throttle: ModelProgressThrottle::new(),
        }
    }

    fn emit_progress(&self, progress: &ModelDownloadProgress, force: bool) {
        update_cache_for_progress(&self.cache, progress);
        if !self.throttle.should_emit(force) {
            return;
        }
        let _ = self.emitter.emit_model_progress(progress);
        if matches!(
            progress.status,
            ModelDownloadStatus::Downloading | ModelDownloadStatus::Verifying
        ) {
            let _ = self
                .emitter
                .emit_phase_changed(TranscribePhase::LoadingModel);
        }
    }

    fn emit_phase(&self, phase: TranscribePhase) {
        emit_transcribe_phase(&self.cache, self.emitter.as_ref(), phase);
    }

    fn report_orchestrator_error(
        &self,
        orch: &mut dyn TranscribeOrchestrator,
        err: &TranscribeError,
    ) {
        orch.fail_model_loading();
        let _ = self.emitter.emit_error(err);
        self.emit_phase(orch.phase());
    }
}

fn progress_emit_is_forced(progress: &ModelDownloadProgress) -> bool {
    matches!(
        progress.status,
        ModelDownloadStatus::Verifying
            | ModelDownloadStatus::Complete
            | ModelDownloadStatus::Failed
    ) || progress.bytes_downloaded == 0
}

fn try_start_if_ready(orch: &mut dyn TranscribeOrchestrator) {
    if orch.phase() == TranscribePhase::Ready {
        let _ = orch.start();
    }
}

fn finish_loaded_model(
    orch_for_model: &Arc<Mutex<dyn TranscribeOrchestrator>>,
    reporter: &ModelLoadReporter,
    path: std::path::PathBuf,
) {
    let mut orch = orch_for_model.lock().expect("lock orchestrator");
    match orch.finish_model_loading(&path) {
        Ok(()) => {
            reporter.cache.clear_progress();
            try_start_if_ready(&mut *orch);
            reporter.emit_phase(orch.phase());
        }
        Err(err) => reporter.report_orchestrator_error(&mut *orch, &err),
    }
}

fn update_cache_for_progress(cache: &TranscribeStatusCache, progress: &ModelDownloadProgress) {
    cache.set_progress(progress);
    if matches!(
        progress.status,
        ModelDownloadStatus::Downloading | ModelDownloadStatus::Verifying
    ) {
        cache.set_phase(TranscribePhase::LoadingModel, 0);
    }
}

fn emit_transcribe_phase(
    cache: &TranscribeStatusCache,
    emitter: &dyn TranscribeEventEmitter,
    phase: TranscribePhase,
) {
    cache.set_phase(phase, 0);
    let _ = emitter.emit_phase_changed(phase);
}

fn parse_capture_phase_event(payload_str: &str) -> CapturePhase {
    if payload_str.contains("\"capturing\"") {
        CapturePhase::Capturing
    } else if payload_str.contains("\"stopping\"") {
        CapturePhase::Stopping
    } else if payload_str.contains("\"error\"") {
        CapturePhase::Error
    } else {
        CapturePhase::Idle
    }
}

fn start_model_load_thread(
    orch_for_model: Arc<Mutex<dyn TranscribeOrchestrator>>,
    model_orchestrator: SharedModelOrchestrator,
    cache: Arc<TranscribeStatusCache>,
    emitter_for_model: Arc<dyn TranscribeEventEmitter>,
) {
    std::thread::spawn(move || {
        let reporter = ModelLoadReporter::new(cache, emitter_for_model);
        reporter.emit_phase(TranscribePhase::LoadingModel);

        if let Err(err) = {
            let mut orch = orch_for_model.lock().expect("lock orchestrator");
            orch.begin_model_loading()
        } {
            let mut orch = orch_for_model.lock().expect("lock orchestrator");
            reporter.report_orchestrator_error(&mut *orch, &err);
            return;
        }

        let acquire_result = {
            let model = model_orchestrator.lock().expect("lock model orchestrator");
            model.ensure_model(|progress| {
                reporter.emit_progress(&progress, progress_emit_is_forced(&progress));
            })
        };

        match acquire_result {
            Ok(path) => finish_loaded_model(&orch_for_model, &reporter, path),
            Err(err) => {
                let mut orch = orch_for_model.lock().expect("lock orchestrator");
                reporter.report_orchestrator_error(&mut *orch, &err);
            }
        }
    });
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
    lifecycle.add_processing_hook(
        Arc::clone(&composed.transcribe_lifecycle) as Arc<dyn CaptureProcessingHook>
    );

    let transcribe_lifecycle = Arc::clone(&composed.transcribe_lifecycle);
    let transcribe_bus = Arc::clone(&composed.transcribe_bus);
    let transcribe_orchestrator = Arc::clone(&composed.transcribe_orchestrator);
    let model_orchestrator = Arc::clone(&composed.model_orchestrator);
    let transcribe_status_cache = Arc::new(TranscribeStatusCache::new());
    let status_cache_for_model_load = Arc::clone(&transcribe_status_cache);
    let app = attach_capture_lifecycle(
        tauri::Builder::default().manage(Arc::clone(&composed.transcribe_lifecycle)),
        lifecycle,
    )
    .setup(move |app| {
        let handle = app.handle().clone();
        let emitter = Arc::new(
            gijirec_presentation::transcribe::TauriTranscribeEventEmitter::new(handle.clone()),
        );
        transcribe_lifecycle.set_emitter(emitter.clone());
        transcribe_bus.set_emitter(Arc::new(
            gijirec_presentation::transcribe::TauriTranscriptBlockEventEmitter::new(handle.clone()),
        ));

        run_capture_app_setup(&handle)
            .map_err(|err| -> Box<dyn std::error::Error> { Box::new(err) })?;

        let hook_for_capture = Arc::clone(&transcribe_lifecycle);
        let _ = handle.listen("audio-capture://phase-changed", move |event| {
            hook_for_capture.on_capture_phase_changed(parse_capture_phase_event(event.payload()));
        });

        start_model_load_thread(
            transcribe_orchestrator,
            model_orchestrator,
            status_cache_for_model_load,
            emitter,
        );

        Ok(())
    })
    .invoke_handler(tauri::generate_handler![
        get_capture_phase,
        get_transcribe_phase,
        get_transcribe_status
    ])
    .manage(pipeline)
    .manage(composed.transcribe_bus)
    .manage(composed.transcribe_orchestrator)
    .manage(transcribe_status_cache)
    .build(tauri::generate_context!())
    .expect("error while building tauri application");

    app.run(|app_handle, event| {
        handle_capture_run_event(app_handle, &event);
    });
}
