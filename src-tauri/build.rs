use std::process::Command;

/// Builds the Swift companion (`helper/helper.swift`) that Tauri bundles as a
/// sidecar: Contents/MacOS/needlex-helper.
fn main() {
    println!("cargo:rerun-if-changed=helper/helper.swift");
    let target = std::env::var("TARGET").expect("TARGET");
    let arch = if target.starts_with("aarch64") { "arm64" } else { "x86_64" };
    std::fs::create_dir_all("binaries").expect("binaries dir");
    let out = format!("binaries/needlex-helper-{target}");
    let status = Command::new("xcrun")
        .args(["swiftc", "-O", "-swift-version", "5", "-target", &format!("{arch}-apple-macos13.0")])
        .args(["helper/helper.swift", "-o", &out])
        .status()
        .expect("swiftc not found – install the Xcode Command Line Tools (xcode-select --install)");
    assert!(status.success(), "building needlex-helper failed");
    tauri_build::build()
}
