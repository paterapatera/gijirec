use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read_boundaries_doc() -> String {
    let path = repo_root()
        .join("docs")
        .join("architecture")
        .join("boundaries.md");
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("missing {}: {err}", path.display()))
}

fn audio_capture_section(source: &str) -> &str {
    source
        .split("## audio-capture")
        .nth(1)
        .expect("boundaries.md must contain an audio-capture section")
}

#[test]
fn boundaries_doc_declares_audio_capture_owns_out_and_allowed_dependencies() {
    let doc = read_boundaries_doc();
    let section = audio_capture_section(&doc);

    assert!(
        section.contains("Owns") || section.contains("所有"),
        "audio-capture section must declare Owns / 所有"
    );
    assert!(
        section.contains("Out") || section.contains("非所有") || section.contains("境界外"),
        "audio-capture section must declare Out / 非所有 / 境界外"
    );
    assert!(
        section.contains("Allowed Dependencies") || section.contains("許可依存"),
        "audio-capture section must declare Allowed Dependencies / 許可依存"
    );
}

#[test]
fn boundaries_doc_excludes_virtual_audio_device_requirement() {
    let doc = read_boundaries_doc();
    let section = audio_capture_section(&doc);

    let mentions_virtual_exclusion = section.contains("仮想オーディオ")
        || section.contains("BlackHole")
        || section.contains("仮想デバイス");
    let mentions_not_required = section.contains("非前提")
        || section.contains("前提条件としない")
        || section.contains("不要");

    assert!(
        mentions_virtual_exclusion,
        "audio-capture section must mention virtual audio device exclusion (仮想オーディオ / BlackHole / 仮想デバイス)"
    );
    assert!(
        mentions_not_required,
        "audio-capture section must state virtual devices are not required (非前提 / 前提条件としない / 不要)"
    );
}

#[test]
fn boundaries_doc_lists_key_capture_components_and_downstream_contracts() {
    let doc = read_boundaries_doc();
    let section = audio_capture_section(&doc);

    for needle in [
        "MicCaptureAdapter",
        "WindowsLoopbackAdapter",
        "MacScreenCaptureKitAdapter",
        "AudioMixer",
        "PcmChunk",
        "audio-capture-pcm",
        "audio-capture-status",
    ] {
        assert!(
            section.contains(needle),
            "audio-capture section must mention {needle}"
        );
    }
}

#[test]
fn boundaries_doc_lists_allowed_os_and_crate_dependencies() {
    let doc = read_boundaries_doc();
    let section = audio_capture_section(&doc);

    for needle in [
        "WASAPI",
        "ScreenCaptureKit",
        "cpal",
        "rubato",
        "rtrb",
        "Tauri",
        "Bun",
    ] {
        assert!(
            section.contains(needle),
            "Allowed Dependencies must mention {needle}"
        );
    }
}

fn audio_device_selection_section(source: &str) -> &str {
    let after = source
        .split("## audio-device-selection")
        .nth(1)
        .expect("boundaries.md must contain an audio-device-selection section");
    after
        .split("\n## ")
        .next()
        .expect("audio-device-selection section must end before the next heading")
}

fn fix_release_transcribe_section(source: &str) -> &str {
    source
        .split("## fix-release-transcribe")
        .nth(1)
        .expect("boundaries.md must contain a fix-release-transcribe section")
}

fn read_adr_0008() -> String {
    let path = repo_root()
        .join("docs")
        .join("architecture")
        .join("adr")
        .join("ADR-0008-model-store-app-data-dir.md");
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("missing {}: {err}", path.display()))
}

fn read_adr_index() -> String {
    let path = repo_root()
        .join("docs")
        .join("architecture")
        .join("adr")
        .join("README.md");
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("missing {}: {err}", path.display()))
}

#[test]
fn boundaries_doc_declares_audio_device_selection_owns_out_and_allowed_dependencies() {
    let doc = read_boundaries_doc();
    let section = audio_device_selection_section(&doc);

    assert!(
        section.contains("Owns") || section.contains("所有"),
        "audio-device-selection section must declare Owns / 所有"
    );
    assert!(
        section.contains("Out") || section.contains("非所有") || section.contains("境界外"),
        "audio-device-selection section must declare Out / 非所有 / 境界外"
    );
    assert!(
        section.contains("Allowed Dependencies") || section.contains("許可依存"),
        "audio-device-selection section must declare Allowed Dependencies / 許可依存"
    );
}

#[test]
fn boundaries_doc_declares_fix_release_transcribe_integration_boundary() {
    let doc = read_boundaries_doc();
    let section = fix_release_transcribe_section(&doc);

    assert!(
        section.contains("Owns") || section.contains("所有"),
        "fix-release-transcribe section must declare Owns / 所有"
    );
    assert!(
        section.contains("Out") || section.contains("境界外"),
        "fix-release-transcribe section must declare Out / 境界外"
    );
    assert!(
        section.contains("Allowed Dependencies") || section.contains("許可依存"),
        "fix-release-transcribe section must declare Allowed Dependencies / 許可依存"
    );

    for needle in [
        "compose.rs",
        "lib.rs",
        "ADR-0008",
        "allow-listen-transcribe-events.toml",
        "block-appended",
        "TranscribeStallWatchdog",
        "app_data_dir",
    ] {
        assert!(
            section.contains(needle),
            "fix-release-transcribe section must mention {needle}"
        );
    }
}

#[test]
fn adr_0008_is_accepted_and_uses_app_data_dir_models() {
    let adr = read_adr_0008();

    assert!(
        adr.contains("**Status**: Accepted"),
        "ADR-0008 must be Accepted"
    );
    assert!(
        adr.contains("{app_data_dir}/models/"),
        "ADR-0008 must canonicalize model path under app_data_dir/models/"
    );
    assert!(
        adr.contains("dirs::data_local_dir()"),
        "ADR-0008 must document removal of dirs::data_local_dir"
    );
    assert!(
        adr.contains("使用しない"),
        "ADR-0008 must reject dirs::data_local_dir for ModelStore"
    );
}

#[test]
fn boundaries_doc_lists_audio_device_selection_owned_components() {
    let doc = read_boundaries_doc();
    let section = audio_device_selection_section(&doc);

    for needle in [
        "DeviceSelectorPanel",
        "restart_with_selection",
        "DeviceSelection",
        "audio-device-selection.md",
        "ホットプラグ",
        "audio-capture-status",
        "SELECTED_MIC_UNAVAILABLE",
    ] {
        assert!(
            section.contains(needle),
            "audio-device-selection Owns must mention {needle}"
        );
    }
}

#[test]
fn boundaries_doc_lists_audio_device_selection_out_of_boundary() {
    let doc = read_boundaries_doc();
    let section = audio_device_selection_section(&doc);

    for needle in ["PcmChunk", "認証", "外部ネットワーク"] {
        assert!(
            section.contains(needle),
            "audio-device-selection Out of Boundary must mention {needle}"
        );
    }
}

#[test]
fn boundaries_doc_lists_audio_device_selection_allowed_dependencies() {
    let doc = read_boundaries_doc();
    let section = audio_device_selection_section(&doc);

    for needle in [
        "TauriLifecycleHook",
        "CaptureOrchestrator",
        "ADR-0001",
        "ADR-0009",
        "audio-capture-pcm",
        "audio-capture-status",
        "audio-device-selection",
    ] {
        assert!(
            section.contains(needle),
            "audio-device-selection Allowed Dependencies must mention {needle}"
        );
    }
}

#[test]
fn adr_index_lists_0008_as_accepted() {
    let index = read_adr_index();

    assert!(
        index.contains("ADR-0008-model-store-app-data-dir.md"),
        "ADR index must list ADR-0008"
    );
    assert!(
        index.contains("| `ADR-0008-model-store-app-data-dir.md`"),
        "ADR index must include ADR-0008 entry row"
    );
    assert!(
        index.contains("Whisper ModelStore"),
        "ADR-0008 index entry must describe ModelStore app_data_dir purpose"
    );

    let row = index
        .lines()
        .find(|line| line.contains("ADR-0008-model-store-app-data-dir.md"))
        .expect("ADR-0008 index row must exist");
    assert!(
        row.contains("Accepted"),
        "ADR-0008 index row must be Accepted"
    );
}
