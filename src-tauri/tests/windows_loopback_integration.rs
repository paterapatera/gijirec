//! Integration Test 1 (Windows): mic + WASAPI loopback simultaneous start.
//!
//! Hardware tests are `#[ignore]` for CI; run locally with:
//! `cargo test --test windows_loopback_integration -- --ignored`

use std::path::PathBuf;

/// Must match `#[ignore = "..."]` on `opens_default_loopback_on_hardware` in windows_loopback.rs.
const LOOPBACK_HARDWARE_IGNORE: &str =
    "CI: requires Windows default WASAPI loopback output device; run with --ignored on local hardware";

/// Must match `#[ignore = "..."]` on `integration_mic_and_wasapi_loopback_reach_capturing_on_hardware` in compose.rs.
const DUAL_CAPTURE_HARDWARE_IGNORE: &str =
    "CI: requires Windows mic permission and default WASAPI loopback output; run with --ignored on local hardware";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_windows_loopback_source() -> String {
    std::fs::read_to_string(
        repo_root().join("crates/gijirec-infrastructure/src/audio/platform/windows_loopback.rs"),
    )
    .expect("windows_loopback.rs must exist")
}

fn read_compose_source() -> String {
    std::fs::read_to_string(repo_root().join("src/compose.rs")).expect("compose.rs must exist")
}

#[test]
fn integration_test_1_documents_windows_wasapi_loopback_ci_skip_in_source() {
    let loopback = read_windows_loopback_source();
    assert!(
        loopback.contains(LOOPBACK_HARDWARE_IGNORE),
        "WINDOWS_LOOPBACK_HARDWARE_SKIP must match loopback hardware ignore reason"
    );
    assert!(
        loopback.contains(&format!(r#"#[ignore = "{LOOPBACK_HARDWARE_IGNORE}"]"#)),
        "opens_default_loopback_on_hardware must declare exact #[ignore] reason"
    );
    assert!(
        loopback.contains("WINDOWS_LOOPBACK_HARDWARE_SKIP"),
        "skip reason must be shared via WINDOWS_LOOPBACK_HARDWARE_SKIP const"
    );

    let compose = read_compose_source();
    assert!(
        compose.contains("integration_mic_and_wasapi_loopback_reach_capturing_on_hardware"),
        "compose must define mic+loopback hardware integration test"
    );
    assert!(
        compose.contains(DUAL_CAPTURE_HARDWARE_IGNORE),
        "dual-capture hardware ignore reason must be documented in compose.rs"
    );
    assert!(
        compose.contains(&format!(r#"#[ignore = "{DUAL_CAPTURE_HARDWARE_IGNORE}"]"#)),
        "integration_mic_and_wasapi_loopback_reach_capturing_on_hardware must declare exact #[ignore] reason"
    );
}
