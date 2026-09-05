use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn src_tauri() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const LAYER_PACKAGES: &[&str] = &[
    "gijirec-domain",
    "gijirec-application",
    "gijirec-infrastructure",
    "gijirec-presentation",
];

fn bylaw_check(config: &Path, manifest: &Path) -> Output {
    let mut command = Command::new("cargo");
    command
        .args(["bylaw", "check", "--config"])
        .arg(config)
        .arg("--manifest-path")
        .arg(manifest);
    for package in LAYER_PACKAGES {
        command.args(["-p", package]);
    }
    command
        .output()
        .unwrap_or_else(|err| panic!("failed to spawn cargo bylaw: {err}"))
}

fn assert_command_available(output: &Output) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("no such command: `bylaw`"),
        "cargo-bylaw must be installed: cargo install cargo-bylaw\n{stderr}"
    );
}

fn write_stub_crate(root: &Path, name: &str, dependencies: &str, lib: &str) {
    let crate_dir = root.join("crates").join(name);
    fs::create_dir_all(crate_dir.join("src")).expect("create stub crate");
    let manifest = format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n{dependencies}"
    );
    fs::write(crate_dir.join("Cargo.toml"), manifest).expect("write stub Cargo.toml");
    fs::write(crate_dir.join("src/lib.rs"), lib).expect("write stub lib.rs");
}

fn layer_violation_workspace() -> PathBuf {
    let root = std::env::temp_dir().join(format!("gijirec-bylaw-violation-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("crates")).expect("create fixture root");
    fs::copy(src_tauri().join("bylaw.toml"), root.join("bylaw.toml")).expect("copy bylaw.toml");
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n",
    )
    .expect("write workspace Cargo.toml");
    write_stub_crate(&root, "gijirec-domain", "", "//! fixture domain\n");
    write_stub_crate(
        &root,
        "gijirec-infrastructure",
        "[dependencies]\ngijirec-domain = { path = \"../gijirec-domain\" }\n",
        "pub use gijirec_domain as domain;\n",
    );
    write_stub_crate(
        &root,
        "gijirec-application",
        "[dependencies]\ngijirec-domain = { path = \"../gijirec-domain\" }\ngijirec-infrastructure = { path = \"../gijirec-infrastructure\" }\n",
        "pub use gijirec_infrastructure as infrastructure;\n",
    );
    write_stub_crate(
        &root,
        "gijirec-presentation",
        "[dependencies]\ngijirec-application = { path = \"../gijirec-application\" }\ngijirec-domain = { path = \"../gijirec-domain\" }\ngijirec-infrastructure = { path = \"../gijirec-infrastructure\" }\n",
        "pub use gijirec_application as application;\n",
    );
    root
}

#[test]
fn bylaw_passes_on_clean_workspace() {
    let root = src_tauri();
    let output = bylaw_check(&root.join("bylaw.toml"), &root.join("Cargo.toml"));
    assert_command_available(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "clean workspace must pass cargo bylaw\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn bylaw_fails_when_application_depends_on_infrastructure() {
    let root = layer_violation_workspace();
    let output = bylaw_check(&root.join("bylaw.toml"), &root.join("Cargo.toml"));
    let _ = fs::remove_dir_all(&root);
    assert_command_available(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "application → infrastructure must fail cargo bylaw\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}
