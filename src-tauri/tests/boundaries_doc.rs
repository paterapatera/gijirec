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
