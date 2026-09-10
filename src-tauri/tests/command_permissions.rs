//! Ensures frontend `invoke()` commands are declared in Tauri ACL permissions.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn push_command_token(allowed: &mut HashSet<String>, token: &str) {
    let cmd = token.trim().trim_matches('"').trim_matches('\'');
    if !cmd.is_empty() {
        allowed.insert(cmd.to_string());
    }
}

fn collect_inline_commands(allowed: &mut HashSet<String>, inline: &str) {
    let rest = inline.trim_end_matches(']');
    for token in rest.split(',') {
        push_command_token(allowed, token);
    }
}

fn process_permission_line(
    allowed: &mut HashSet<String>,
    in_commands_allow: &mut bool,
    line: &str,
) {
    let trimmed = line.trim();
    if let Some(inline) = trimmed.strip_prefix("commands.allow = [") {
        if trimmed.ends_with(']') && !inline.is_empty() {
            collect_inline_commands(allowed, inline);
            return;
        }
        *in_commands_allow = true;
        return;
    }
    if trimmed.starts_with("commands.allow") {
        *in_commands_allow = true;
        return;
    }
    if !*in_commands_allow {
        return;
    }
    if trimmed == "]" {
        *in_commands_allow = false;
        return;
    }
    push_command_token(allowed, trimmed.trim_end_matches(','));
}

fn read_allowed_commands() -> HashSet<String> {
    let permissions_dir = manifest_dir().join("permissions");
    let mut allowed = HashSet::new();

    for entry in fs::read_dir(&permissions_dir).expect("read permissions dir") {
        let path = entry.expect("permission entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let contents = fs::read_to_string(&path).expect("read permission toml");
        let mut in_commands_allow = false;
        for line in contents.lines() {
            process_permission_line(&mut allowed, &mut in_commands_allow, line);
        }
    }

    allowed
}

/// Commands the React layer invokes via `@tauri-apps/api/core` `invoke`.
const FRONTEND_INVOKE_COMMANDS: &[&str] = &[
    "get_capture_phase",
    "get_transcribe_phase",
    "get_transcribe_status",
    "get_transcribe_settings",
    "set_transcribe_model_variant",
    "save_transcript_session",
    "get_editor_settings",
    "set_editor_settings",
    "pick_save_directory",
    "list_audio_devices",
    "get_device_selection",
    "set_device_selection",
    "set_audio_device_ui_visible",
    "get_capture_audio_controls",
    "set_capture_audio_controls",
];

#[test]
fn frontend_invoke_commands_are_allowed_in_tauri_permissions() {
    let allowed = read_allowed_commands();

    for command in FRONTEND_INVOKE_COMMANDS {
        assert!(
            allowed.contains(*command),
            "missing Tauri ACL allow for frontend invoke command: {command}"
        );
    }
}
