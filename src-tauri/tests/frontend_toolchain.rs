use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn package_json() -> String {
    let path = repo_root().join("package.json");
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("missing {}: {err}", path.display()))
}

fn json_string_field(source: &str, key: &str) -> Option<String> {
    let quoted_key = format!("\"{key}\"");
    let after_key = source.split(&quoted_key).nth(1)?;
    let after_colon = after_key.split(':').nth(1)?;
    let mut parts = after_colon.split('"');
    let _before_value = parts.next()?;
    Some(parts.next()?.to_string())
}

fn script_value(name: &str) -> Option<String> {
    let raw = package_json();
    let scripts = raw.split("\"scripts\"").nth(1)?;
    json_string_field(scripts, name)
}

#[test]
fn package_json_defines_dev_build_and_check() {
    for name in ["dev", "build", "check"] {
        let value = script_value(name).unwrap_or_else(|| panic!("scripts.{name} must be present"));
        assert!(!value.is_empty(), "scripts.{name} must not be empty");
    }
}

#[test]
fn package_json_scripts_do_not_use_npm_run() {
    let raw = package_json();
    let scripts = raw
        .split("\"scripts\"")
        .nth(1)
        .expect("package.json must have scripts");
    assert!(
        !scripts.contains("npm run"),
        "package.json scripts must use bun, not npm run"
    );
}

#[test]
fn bun_lockfile_is_the_record() {
    assert!(
        repo_root().join("bun.lock").is_file(),
        "bun.lock must exist as the lockfile of record"
    );
    assert!(
        !repo_root().join("package-lock.json").is_file(),
        "package-lock.json must not remain as the lockfile of record"
    );
}

#[test]
fn vite_typescript_entry_exists() {
    let root = repo_root();
    assert!(root.join("src/main.ts").is_file(), "src/main.ts must exist");
    assert!(root.join("index.html").is_file(), "index.html must exist");
    assert!(
        Path::new(&root.join("vite.config.ts")).is_file(),
        "vite.config.ts must exist"
    );
}

fn first_semver_major(spec: &str) -> u32 {
    let digits: String = spec
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    digits
        .parse()
        .unwrap_or_else(|_| panic!("no major version in {spec}"))
}

fn dev_dependency_spec(name: &str) -> String {
    let raw = package_json();
    let deps = raw
        .split("\"devDependencies\"")
        .nth(1)
        .expect("package.json must have devDependencies");
    json_string_field(deps, name)
        .unwrap_or_else(|| panic!("devDependencies.{name} must be present"))
}

#[test]
fn plugin_react_major_matches_vite() {
    let vite_major = first_semver_major(&dev_dependency_spec("vite"));
    let plugin_major = first_semver_major(&dev_dependency_spec("@vitejs/plugin-react"));
    match vite_major {
        7 => assert_eq!(
            plugin_major, 5,
            "@vitejs/plugin-react 6+ requires Vite 8 (vite/internal); keep 5.x with Vite 7"
        ),
        8.. => assert!(
            plugin_major >= 6,
            "Vite 8 pairs with @vitejs/plugin-react 6+ (peer vite ^8)"
        ),
        other => panic!("unsupported vite major {other} in package.json"),
    }
}
