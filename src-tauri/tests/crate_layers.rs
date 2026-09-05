use std::path::PathBuf;

fn crates_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("crates")
}

fn crate_src(name: &str) -> PathBuf {
    crates_dir().join(name).join("src")
}

fn read_file(path: PathBuf) -> String {
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("missing {}: {err}", path.display()))
}

const REQUIRED_FILES: &[(&str, &[&str])] = &[
    (
        "gijirec-domain",
        &[
            "lib.rs",
            "audio/mod.rs",
            "audio/pcm_chunk.rs",
            "audio/phase.rs",
            "audio/error.rs",
        ],
    ),
    (
        "gijirec-application",
        &[
            "lib.rs",
            "capture/mod.rs",
            "capture/orchestrator.rs",
            "capture/mixer.rs",
            "capture/chunk_emitter.rs",
        ],
    ),
    (
        "gijirec-infrastructure",
        &[
            "lib.rs",
            "audio/mod.rs",
            "audio/mic_capture.rs",
            "audio/resampler.rs",
            "audio/platform/mod.rs",
            "audio/platform/windows_loopback.rs",
            "audio/platform/macos_sck_audio.rs",
        ],
    ),
    (
        "gijirec-presentation",
        &[
            "lib.rs",
            "tauri/mod.rs",
            "tauri/lifecycle.rs",
            "tauri/events.rs",
            "tauri/pcm_bus.rs",
        ],
    ),
];

#[test]
fn designed_module_files_exist() {
    for (crate_name, files) in REQUIRED_FILES {
        for file in *files {
            let path = crate_src(crate_name).join(file);
            assert!(path.is_file(), "missing {}", path.display());
        }
    }
}

#[test]
fn lib_rs_declares_designed_root_modules() {
    let domain = read_file(crate_src("gijirec-domain").join("lib.rs"));
    assert!(
        domain.contains("mod audio"),
        "domain lib.rs must declare audio"
    );
    let application = read_file(crate_src("gijirec-application").join("lib.rs"));
    assert!(
        application.contains("mod capture"),
        "application lib.rs must declare capture"
    );
    let infrastructure = read_file(crate_src("gijirec-infrastructure").join("lib.rs"));
    assert!(
        infrastructure.contains("mod audio"),
        "infrastructure lib.rs must declare audio"
    );
    let presentation = read_file(crate_src("gijirec-presentation").join("lib.rs"));
    assert!(
        presentation.contains("mod tauri"),
        "presentation lib.rs must declare tauri"
    );
}

#[test]
fn domain_cargo_toml_has_no_outer_crate_deps() {
    let manifest = read_file(crates_dir().join("gijirec-domain").join("Cargo.toml"));
    for forbidden in [
        "gijirec-application",
        "gijirec-infrastructure",
        "gijirec-presentation",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "domain must not depend on {forbidden}"
        );
    }
}
