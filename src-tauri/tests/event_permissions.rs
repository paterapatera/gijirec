//! Ensures frontend `listen()` targets are declared in Tauri ACL permissions.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_permission_events() -> HashSet<String> {
    let permissions_dir = manifest_dir().join("permissions");
    let mut allowed = HashSet::new();

    for entry in fs::read_dir(&permissions_dir).expect("read permissions dir") {
        let path = entry.expect("permission entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let contents = fs::read_to_string(&path).expect("read permission toml");
        for line in contents.lines() {
            let trimmed = line.trim();
            if let Some(event) = trimmed.strip_prefix("event = ") {
                let unquoted = event.trim_matches('"');
                allowed.insert(unquoted.to_string());
            }
        }
    }

    allowed
}

/// Events the React hooks subscribe to via `@tauri-apps/api/event` `listen`.
const FRONTEND_LISTEN_EVENTS: &[&str] = &[
    "audio-capture://phase-changed",
    "audio-capture://error",
    "whisper-transcribe://phase-changed",
    "whisper-transcribe://model-progress",
    "whisper-transcribe://error",
    "whisper-transcribe://block-appended",
    "audio-device-selection://devices-changed",
    "audio-device-selection://selection-changed",
];

#[test]
fn frontend_listen_events_are_allowed_in_tauri_permissions() {
    let allowed = read_permission_events();

    for event in FRONTEND_LISTEN_EVENTS {
        assert!(
            allowed.contains(*event),
            "missing Tauri ACL allow for frontend listen target: {event}"
        );
    }
}
