use std::path::PathBuf;

fn tauri_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_tauri_conf() -> String {
    let path = tauri_dir().join("tauri.conf.json");
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("missing {}: {err}", path.display()))
}

fn read_info_plist() -> String {
    let path = tauri_dir().join("Info.plist");
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("missing {}: {err}", path.display()))
}

fn read_infrastructure_cargo_toml() -> String {
    let path = tauri_dir()
        .join("crates")
        .join("gijirec-infrastructure")
        .join("Cargo.toml");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("missing {}: {err}", path.display()))
}

fn plist_string_value(source: &str, key: &str) -> Option<String> {
    let key_tag = format!("<key>{key}</key>");
    let after_key = source.split(&key_tag).nth(1)?;
    let after_string_tag = after_key.split("<string>").nth(1)?;
    let value = after_string_tag.split("</string>").next()?;
    Some(value.trim().to_string())
}

fn contains_japanese(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(ch,
            '\u{3040}'..='\u{309F}'   // Hiragana
            | '\u{30A0}'..='\u{30FF}' // Katakana
            | '\u{4E00}'..='\u{9FFF}' // CJK Unified Ideographs
        )
    })
}

fn json_string_field(source: &str, key: &str) -> Option<String> {
    let quoted_key = format!("\"{key}\"");
    let after_key = source.split(&quoted_key).nth(1)?;
    let after_colon = after_key.split(':').nth(1)?;
    let mut parts = after_colon.split('"');
    let _before_value = parts.next()?;
    Some(parts.next()?.to_string())
}

#[test]
fn info_plist_declares_microphone_usage_description_in_japanese() {
    let plist = read_info_plist();
    let description = plist_string_value(&plist, "NSMicrophoneUsageDescription")
        .expect("Info.plist must declare NSMicrophoneUsageDescription");
    assert!(
        !description.is_empty(),
        "NSMicrophoneUsageDescription must not be empty"
    );
    assert!(
        contains_japanese(&description),
        "NSMicrophoneUsageDescription must be Japanese: {description}"
    );
    assert!(
        description.to_ascii_lowercase().contains("gijirec"),
        "NSMicrophoneUsageDescription must mention gijirec: {description}"
    );
    assert!(
        description.contains("マイク"),
        "NSMicrophoneUsageDescription must mention microphone capture: {description}"
    );
}

#[test]
fn info_plist_declares_screen_capture_usage_description_in_japanese() {
    let plist = read_info_plist();
    let description = plist_string_value(&plist, "NSScreenCaptureUsageDescription")
        .expect("Info.plist must declare NSScreenCaptureUsageDescription");
    assert!(
        !description.is_empty(),
        "NSScreenCaptureUsageDescription must not be empty"
    );
    assert!(
        contains_japanese(&description),
        "NSScreenCaptureUsageDescription must be Japanese: {description}"
    );
    assert!(
        description.contains("画面とシステムオーディオ"),
        "NSScreenCaptureUsageDescription must mention screen and system audio: {description}"
    );
}

#[test]
fn tauri_conf_references_info_plist_and_minimum_macos_13() {
    let conf = read_tauri_conf();
    let info_plist = json_string_field(&conf, "infoPlist")
        .expect("bundle.macOS.infoPlist must reference Info.plist");
    assert!(
        info_plist.contains("Info.plist"),
        "infoPlist must point at Info.plist, got {info_plist}"
    );
    let minimum = json_string_field(&conf, "minimumSystemVersion")
        .expect("bundle.macOS.minimumSystemVersion must be set for SCK");
    let major: u32 = minimum
        .split('.')
        .next()
        .and_then(|part| part.parse().ok())
        .unwrap_or(0);
    assert!(
        major >= 13,
        "minimumSystemVersion must be >= 13 for ScreenCaptureKit, got {minimum}"
    );
}

#[test]
fn infrastructure_cargo_toml_lists_cpal_and_screencapturekit() {
    let cargo_toml = read_infrastructure_cargo_toml();
    assert!(
        cargo_toml.contains("cpal"),
        "gijirec-infrastructure must depend on cpal for microphone capture"
    );
    assert!(
        cargo_toml.contains("screencapturekit"),
        "gijirec-infrastructure must depend on screencapturekit for macOS system audio"
    );
    assert!(
        cargo_toml.contains("[target.'cfg(target_os = \"macos\")'.dependencies]"),
        "screencapturekit must be gated to macOS target"
    );
}

#[test]
fn presentation_crate_links_infrastructure_native_deps_into_host() {
    let path = tauri_dir()
        .join("crates")
        .join("gijirec-presentation")
        .join("Cargo.toml");
    let cargo_toml = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("missing {}: {err}", path.display()));
    assert!(
        cargo_toml.contains("gijirec-infrastructure"),
        "presentation crate must depend on gijirec-infrastructure so native deps link into the host"
    );
}
