use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_lib_rs() -> String {
    std::fs::read_to_string(repo_root().join("src").join("lib.rs")).expect("src/lib.rs must exist")
}

#[test]
fn lib_rs_wires_capture_lifecycle_and_exit_handler() {
    let source = read_lib_rs();
    assert!(
        source.contains("attach_capture_lifecycle"),
        "composition root must attach capture lifecycle"
    );
    assert!(
        source.contains("handle_capture_run_event"),
        "composition root must handle RunEvent::Exit"
    );
    assert!(
        source.contains("build_capture_stack"),
        "composition root must build capture stack"
    );
}

fn read_capture_ports_rs() -> String {
    std::fs::read_to_string(repo_root().join("src").join("capture_ports.rs"))
        .expect("src/capture_ports.rs must exist")
}

#[test]
fn lib_rs_uses_platform_system_adapter() {
    let source = read_capture_ports_rs();
    #[cfg(target_os = "windows")]
    {
        assert!(
            source.contains("WindowsLoopbackAdapter"),
            "Windows capture ports must use WASAPI loopback adapter"
        );
    }
    #[cfg(target_os = "macos")]
    {
        assert!(
            source.contains("MacScreenCaptureKitAdapter"),
            "macOS capture ports must use SCK adapter"
        );
    }
}
