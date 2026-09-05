//! Integration Test 2 (macOS): mic + ScreenCaptureKit system audio start.
//!
//! Hardware tests are `#[ignore]` for CI; run locally with:
//! `cargo test --test macos_sck_integration -- --ignored`

use std::path::PathBuf;

/// Must match `#[ignore = "..."]` on `opens_sck_audio_on_hardware` in macos_sck_audio.rs.
const SCK_HARDWARE_IGNORE: &str = "CI: requires macOS 13+ ScreenCaptureKit screen recording permission; run with --ignored on local hardware";

/// Must match `#[ignore = "..."]` on `integration_mic_and_sck_reach_capturing_on_hardware` in compose.rs.
const DUAL_CAPTURE_HARDWARE_IGNORE: &str = "CI: requires macOS mic permission and ScreenCaptureKit screen recording permission; run with --ignored on local hardware";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_macos_sck_source() -> String {
    std::fs::read_to_string(
        repo_root().join("crates/gijirec-infrastructure/src/audio/platform/macos_sck_audio.rs"),
    )
    .expect("macos_sck_audio.rs must exist")
}

fn read_compose_source() -> String {
    std::fs::read_to_string(repo_root().join("src/compose.rs")).expect("compose.rs must exist")
}

#[test]
fn integration_test_2_documents_macos_sck_ci_skip_in_source() {
    let sck = read_macos_sck_source();
    assert!(
        sck.contains("ScreenCaptureKit"),
        "MacScreenCaptureKitAdapter tests must document ScreenCaptureKit"
    );
    assert!(
        sck.contains("permission"),
        "MacScreenCaptureKitAdapter tests must document permission requirements"
    );
    assert!(
        sck.contains(SCK_HARDWARE_IGNORE),
        "MACOS_SCK_HARDWARE_SKIP must match SCK hardware ignore reason"
    );
    assert!(
        sck.contains(&format!(r#"#[ignore = "{SCK_HARDWARE_IGNORE}"]"#)),
        "opens_sck_audio_on_hardware must declare exact #[ignore] reason"
    );
    assert!(
        sck.contains("MACOS_SCK_HARDWARE_SKIP"),
        "skip reason must be shared via MACOS_SCK_HARDWARE_SKIP const"
    );

    let compose = read_compose_source();
    assert!(
        compose.contains("integration_mic_and_sck_reach_capturing_on_hardware"),
        "compose must define mic+SCK hardware integration test"
    );
    assert!(
        compose.contains("ScreenCaptureKit"),
        "compose integration test must reference ScreenCaptureKit boundary"
    );
    assert!(
        compose.contains(DUAL_CAPTURE_HARDWARE_IGNORE),
        "dual-capture hardware ignore reason must be documented in compose.rs"
    );
    assert!(
        compose.contains(&format!(r#"#[ignore = "{DUAL_CAPTURE_HARDWARE_IGNORE}"]"#)),
        "integration_mic_and_sck_reach_capturing_on_hardware must declare exact #[ignore] reason"
    );
}
