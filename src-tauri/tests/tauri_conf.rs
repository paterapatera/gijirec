use std::path::PathBuf;

fn tauri_conf_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json")
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
fn before_dev_command_uses_bun_run_dev() {
    let path = tauri_conf_path();
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("missing {}: {err}", path.display()));
    let value =
        json_string_field(&raw, "beforeDevCommand").expect("beforeDevCommand must be present");
    assert_eq!(value, "bun run dev");
}

#[test]
fn before_build_command_uses_bun_run_build() {
    let path = tauri_conf_path();
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("missing {}: {err}", path.display()));
    let value =
        json_string_field(&raw, "beforeBuildCommand").expect("beforeBuildCommand must be present");
    assert_eq!(value, "bun run build");
}
