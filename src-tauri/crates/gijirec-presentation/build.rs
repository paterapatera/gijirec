//! macOS: link Swift Concurrency rpath for test binaries that pull ScreenCaptureKit.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    if let Some(dir) = swift_concurrency_search_dir() {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}");
        println!("cargo:rerun-if-changed=build.rs");
    }
}

fn swift_concurrency_search_dir() -> Option<String> {
    for dir in candidate_dirs() {
        let lib = format!("{dir}/libswift_Concurrency.dylib");
        if std::path::Path::new(&lib).is_file() {
            return Some(dir);
        }
    }
    None
}

fn candidate_dirs() -> Vec<String> {
    let mut dirs = vec!["/usr/lib/swift".to_string()];

    if let Ok(output) = std::process::Command::new("xcrun")
        .args(["--find", "swift"])
        .output()
        && output.status.success()
    {
        let swift = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if let Some(toolchain_root) = swift
            .split("/usr/bin/swift")
            .next()
            .filter(|prefix| !prefix.is_empty())
        {
            dirs.push(format!("{toolchain_root}/usr/lib/swift/macosx"));
            dirs.push(format!("{toolchain_root}/usr/lib/swift-5.5/macosx"));
        }
    }

    dirs.push("/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/macosx".to_string());
    dirs.push(
        "/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift-5.5/macosx".to_string(),
    );

    dirs
}
