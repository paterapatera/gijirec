pub mod capture_observability;
mod capture_ports;
mod capture_processing;
pub mod commands;
mod compose;
pub mod device_selection_observability;
pub mod editor_observability;
pub mod logging;
pub mod transcribe_observability;

#[cfg(debug_assertions)]
pub mod test_support {
    pub use crate::capture_ports::{CaptureStreamHandles, SyntheticMicPort, SyntheticSystemPort};
    pub use crate::capture_processing::CapturePipelineState;

    /// Shared stream handles for integration tests (discards default port adapters).
    pub fn new_stream_handles() -> CaptureStreamHandles {
        let (streams, _, _) = CaptureStreamHandles::new_pair();
        streams
    }

    /// Capture pipeline wired to shared stream handles for integration tests.
    pub fn new_pipeline(streams: CaptureStreamHandles) -> CapturePipelineState {
        CapturePipelineState::new(streams)
    }

    pub fn notify_stream_disconnected(streams: &CaptureStreamHandles) {
        streams.notify_stream_disconnected();
    }

    /// Starts PCM processing; panics only on invariant failure (test helper).
    pub fn start_processing(pipeline: &CapturePipelineState) {
        pipeline
            .start_processing()
            .expect("start_processing in integration test");
    }
}

use capture_observability::TracingCaptureObservability;
use commands::device_selection::{
    get_device_selection, list_audio_devices, set_audio_device_ui_visible, set_device_selection,
};
use commands::editor::{
    get_editor_settings, pick_save_directory, save_transcript_session, set_editor_settings,
};
use commands::transcribe_settings::{
    get_transcribe_settings, set_transcribe_model_variant,
};
use commands::{EditorState, TranscribeSettingsState, get_capture_phase, get_transcribe_phase, get_transcribe_status};
use compose::{SharedModelOrchestrator, build_capture_stack, inject_model_stack_shared};
use editor_observability::TracingEditorObservability;
use gijirec_presentation::application::editor::SettingsService;
use gijirec_presentation::application::transcribe::TranscribeSettingsService;
use gijirec_presentation::application::transcribe::orchestrator::TranscribeOrchestrator;
use gijirec_presentation::domain::transcribe::WhisperModelVariant;
use gijirec_presentation::transcribe::apply_transcribe_model_variant_impl;
use gijirec_presentation::application::transcribe::ports::{
    ModelDownloadProgress, ModelDownloadStatus,
};
use gijirec_presentation::domain::audio::CapturePhase;
use gijirec_presentation::domain::transcribe::{TranscribeError, TranscribePhase};
use gijirec_presentation::editor::set_editor_observability;
use gijirec_presentation::tauri::lifecycle::{
    CaptureLifecycleState, CaptureProcessingHook, OsCapturePlatformSupport,
    OsUnsupportedPlatformNotifier, attach_capture_lifecycle, handle_capture_run_event,
    run_capture_app_setup,
};
use gijirec_presentation::tauri::observability::{init_session_id, session_id, set_observability};
use gijirec_presentation::transcribe::TranscribeEventEmitter;
use gijirec_presentation::transcribe::TranscribeStatusCache;
use gijirec_presentation::transcribe::observability::set_transcribe_observability;
use logging::{
    ReleaseLogConfig, install_global_subscriber, parse_release_log_config_from_env, run_session_id,
    setup_release_file_logging,
};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{Listener, Manager};
use transcribe_observability::TracingTranscribeObservability;

const MODEL_PROGRESS_EMIT_INTERVAL: Duration = Duration::from_millis(250);

fn init_tracing(config: &ReleaseLogConfig) {
    install_global_subscriber(config);
    set_observability(Box::new(TracingCaptureObservability));
    set_transcribe_observability(Box::new(TracingTranscribeObservability));
    set_editor_observability(Box::new(TracingEditorObservability));
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

pub(crate) struct ModelLoadReporter {
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
        gijirec_presentation::transcribe::observability::log_transcribe_error(err);
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

fn finish_loaded_model(
    orch_for_model: &Arc<Mutex<dyn TranscribeOrchestrator>>,
    lifecycle: &gijirec_presentation::transcribe::TranscribeLifecycleHook,
    reporter: &ModelLoadReporter,
    path: std::path::PathBuf,
) {
    let finish_result = {
        let mut orch = orch_for_model.lock().expect("lock orchestrator");
        orch.finish_model_loading(&path)
    };
    match finish_result {
        Ok(()) => {
            reporter.cache.clear_progress();
            lifecycle.on_model_ready();
            let phase = orch_for_model.lock().expect("lock orchestrator").phase();
            reporter.emit_phase(phase);
        }
        Err(err) => {
            let mut orch = orch_for_model.lock().expect("lock orchestrator");
            reporter.report_orchestrator_error(&mut *orch, &err);
        }
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
    gijirec_presentation::transcribe::observability::log_phase_transition(phase);
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
    reporter: ModelLoadReporter,
    lifecycle: Arc<gijirec_presentation::transcribe::TranscribeLifecycleHook>,
) {
    std::thread::spawn(move || {
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
            Ok(path) => {
                {
                    let variant = model_orchestrator
                        .lock()
                        .expect("lock model orchestrator")
                        .selected_variant();
                    model_orchestrator
                        .lock()
                        .expect("lock model orchestrator")
                        .mark_active_variant(variant);
                    gijirec_presentation::transcribe::observability::log_model_variant_applied(
                        variant,
                    );
                }
                finish_loaded_model(&orch_for_model, lifecycle.as_ref(), &reporter, path);
            }
            Err(err) => {
                let mut orch = orch_for_model.lock().expect("lock orchestrator");
                reporter.report_orchestrator_error(&mut *orch, &err);
            }
        }
    });
}

pub(crate) fn spawn_transcribe_model_variant_apply(
    model_orchestrator: SharedModelOrchestrator,
    transcribe_orchestrator: Arc<Mutex<dyn TranscribeOrchestrator>>,
    cache: Arc<TranscribeStatusCache>,
    emitter: Arc<dyn TranscribeEventEmitter>,
    model_variant: WhisperModelVariant,
) {
    let reporter = ModelLoadReporter::new(cache, emitter);
    std::thread::spawn(move || {
        if let Err(err) = apply_transcribe_model_variant_impl(
            &model_orchestrator,
            &transcribe_orchestrator,
            model_variant,
            |progress| reporter.emit_progress(&progress, progress_emit_is_forced(&progress)),
        ) {
            let mut orch = transcribe_orchestrator.lock().expect("lock orchestrator");
            reporter.report_orchestrator_error(&mut *orch, &err);
        } else {
            reporter.emit_phase(
                transcribe_orchestrator
                    .lock()
                    .expect("lock orchestrator")
                    .phase(),
            );
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let release_log_config = parse_release_log_config_from_env();
    init_tracing(&release_log_config);
    init_session_id();
    let release_log_config_for_setup = release_log_config;
    let composed = build_capture_stack();
    let lifecycle = Arc::new(CaptureLifecycleState::new(
        Arc::clone(&composed.orchestrator),
        Arc::clone(&composed.device_selection),
        Arc::new(OsCapturePlatformSupport),
        Arc::new(OsUnsupportedPlatformNotifier),
    ));
    let pipeline = Arc::clone(&composed.pipeline);
    let lifecycle_for_stream = Arc::clone(&lifecycle);
    pipeline.set_stream_disconnect_handler(Arc::new(move || {
        lifecycle_for_stream.handle_stream_disconnected();
    }));
    let device_selection = Arc::clone(&composed.device_selection);
    let device_selection_events = Arc::clone(&composed.device_selection_events);
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
        tauri::Builder::default()
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_fs::init())
            .manage(Arc::clone(&composed.transcribe_lifecycle)),
        lifecycle,
    )
    .setup(move |app| {
        let handle = app.handle().clone();
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|err| -> Box<dyn std::error::Error> { Box::new(err) })?;

        if let Some(log_guard) = setup_release_file_logging(
            &app_data_dir,
            &release_log_config_for_setup,
            &run_session_id(session_id()),
        ) {
            app.manage(log_guard);
        }

        app.manage(EditorState {
            settings_service: Arc::new(SettingsService::new(app_data_dir.clone())),
        });

        inject_model_stack_shared(&model_orchestrator, app_data_dir.clone());

        let transcribe_settings_service =
            Arc::new(TranscribeSettingsService::new(app_data_dir));
        let settings_load = transcribe_settings_service.load();
        if let Some(issue) = settings_load.issue {
            tracing::warn!(
                target: gijirec_presentation::transcribe::observability::TRANSCRIBE_LOG_TARGET,
                issue = issue.message_ja(),
                "transcribe settings load used defaults"
            );
        }
        {
            let mut model = model_orchestrator.lock().expect("lock model orchestrator");
            model.initialize_selected_variant(settings_load.settings.model_variant);
            gijirec_presentation::transcribe::observability::log_model_variant_selected(
                settings_load.settings.model_variant,
            );
        }
        app.manage(TranscribeSettingsState {
            settings_service: transcribe_settings_service,
            model_orchestrator: Arc::clone(&model_orchestrator),
        });

        let emitter: Arc<dyn TranscribeEventEmitter> = Arc::new(
            gijirec_presentation::transcribe::TauriTranscribeEventEmitter::new(handle.clone()),
        );
        transcribe_lifecycle.set_emitter(Arc::clone(&emitter));
        app.manage(Arc::clone(&emitter));
        transcribe_bus.set_emitter(Arc::new(
            gijirec_presentation::transcribe::TauriTranscriptBlockEventEmitter::new(handle.clone()),
        ));
        device_selection_events.set_emitter(Arc::new(
            gijirec_presentation::tauri::device_selection::TauriDeviceSelectionEventEmitter::new(
                handle.clone(),
            ),
        ));

        run_capture_app_setup(&handle)?;

        let hook_for_capture = Arc::clone(&transcribe_lifecycle);
        let _ = handle.listen("audio-capture://phase-changed", move |event| {
            hook_for_capture.on_capture_phase_changed(parse_capture_phase_event(event.payload()));
        });

        start_model_load_thread(
            transcribe_orchestrator,
            model_orchestrator,
            ModelLoadReporter::new(status_cache_for_model_load, emitter),
            Arc::clone(&transcribe_lifecycle),
        );

        Ok(())
    })
    .invoke_handler(tauri::generate_handler![
        get_capture_phase,
        get_transcribe_phase,
        get_transcribe_status,
        save_transcript_session,
        get_editor_settings,
        set_editor_settings,
        pick_save_directory,
        get_transcribe_settings,
        set_transcribe_model_variant,
        list_audio_devices,
        get_device_selection,
        set_device_selection,
        set_audio_device_ui_visible,
    ])
    .manage(pipeline)
    .manage(device_selection)
    .manage(composed.transcribe_bus)
    .manage(composed.transcribe_orchestrator)
    .manage(transcribe_status_cache)
    .build(tauri::generate_context!())
    .expect("error while building tauri application");

    app.run(|app_handle, event| {
        handle_capture_run_event(app_handle, &event);
    });
}

#[cfg(test)]
mod setup_tests {
    fn read_lib_source() -> String {
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .expect("lib.rs must exist")
    }

    fn setup_block(source: &str) -> &str {
        source
            .split(".setup(")
            .nth(1)
            .and_then(|rest| rest.split("Ok(())").next())
            .expect("lib.rs must define a Tauri setup closure")
    }

    #[test]
    fn setup_injects_model_stack_before_start_model_load_thread() {
        let source = read_lib_source();
        let setup = setup_block(&source);

        assert!(
            setup.contains("inject_model_stack"),
            "Tauri setup must call inject_model_stack before model load"
        );

        let inject_pos = setup
            .find("inject_model_stack")
            .expect("inject_model_stack must appear in setup");
        let load_pos = setup
            .find("start_model_load_thread")
            .expect("start_model_load_thread must appear in setup");
        assert!(
            inject_pos < load_pos,
            "inject_model_stack must run before start_model_load_thread"
        );
    }

    #[test]
    fn setup_does_not_abort_on_capture_startup_failure() {
        let source = read_lib_source();
        let setup = setup_block(&source);
        assert!(
            !setup.contains("run_capture_app_setup(&handle)\n            .map_err"),
            "capture startup failure must not fail Tauri setup (run_capture_app_setup swallows orchestrator errors)"
        );
    }

    #[test]
    fn setup_passes_same_app_data_dir_to_settings_and_model_stack() {
        let source = read_lib_source();
        let setup = setup_block(&source);

        assert!(
            setup.contains("SettingsService::new(app_data_dir"),
            "setup must construct SettingsService from resolved app_data_dir"
        );
        assert!(
            setup.contains("inject_model_stack") && setup.contains("app_data_dir"),
            "setup must pass resolved app_data_dir into inject_model_stack"
        );
    }

    #[test]
    fn model_load_completion_starts_via_lifecycle_hook() {
        let source = read_lib_source();
        let production = source
            .split("mod setup_tests")
            .next()
            .expect("lib.rs must define setup_tests");
        assert!(
            production.contains("lifecycle.on_model_ready()"),
            "model load completion must start transcribe through TranscribeLifecycleHook so the stall watchdog is armed"
        );
        assert!(
            !production.contains("try_start_if_ready"),
            "host must not start the orchestrator while bypassing the lifecycle hook"
        );
        assert!(
            production.contains("start_model_load_thread")
                && production.contains("transcribe_lifecycle"),
            "start_model_load_thread must receive the transcribe lifecycle hook"
        );
    }
}
