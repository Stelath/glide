use std::{env, fs, path::Path, path::PathBuf, process::Command};

const DEFAULT_APPLE_TEAM_ID: &str = "3C99V29U26";

fn main() {
    println!("cargo:rerun-if-changed=macos/GlideAppleHelper.swift");
    println!("cargo:rerun-if-env-changed=APPLE_CODESIGN_IDENTITY");
    println!("cargo:rerun-if-env-changed=GLIDE_APPLE_CODESIGN_IDENTITY");
    println!("cargo:rerun-if-env-changed=APPLE_TEAM_ID");
    println!("cargo:rerun-if-env-changed=GLIDE_APPLE_TEAM_ID");

    let target = env::var("TARGET").unwrap_or_default();
    if !target.contains("apple-darwin") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let source = manifest_dir.join("macos").join("GlideAppleHelper.swift");
    let helper = out_dir.join("GlideAppleHelper");
    let helper_info = out_dir.join("GlideAppleHelper-Info.plist");
    let module_cache = out_dir.join("SwiftModuleCache");

    fs::create_dir_all(&out_dir).expect("failed to create build output directory");
    fs::create_dir_all(&module_cache).expect("failed to create Swift module cache directory");
    fs::write(
        &helper_info,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>GlideAppleHelper</string>
  <key>CFBundleIdentifier</key>
  <string>com.glide.app.apple-helper</string>
  <key>CFBundleName</key>
  <string>Glide Apple Helper</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>NSSpeechRecognitionUsageDescription</key>
  <string>Glide uses Apple Speech for local on-device dictation when selected.</string>
</dict>
</plist>
"#,
    )
    .expect("failed to write GlideAppleHelper Info.plist");

    let status = Command::new("xcrun")
        .args([
            "swiftc",
            source.to_str().unwrap(),
            "-parse-as-library",
            "-O",
            "-module-cache-path",
            module_cache.to_str().unwrap(),
            "-framework",
            "Foundation",
            "-framework",
            "AVFoundation",
            "-framework",
            "Speech",
            "-framework",
            "FoundationModels",
            "-framework",
            "Security",
            "-Xlinker",
            "-sectcreate",
            "-Xlinker",
            "__TEXT",
            "-Xlinker",
            "__info_plist",
            "-Xlinker",
            helper_info.to_str().unwrap(),
            "-o",
            helper.to_str().unwrap(),
        ])
        .status()
        .expect("failed to invoke xcrun swiftc for GlideAppleHelper");

    if !status.success() {
        panic!("failed to build GlideAppleHelper");
    }

    maybe_codesign_helper(&helper);

    println!(
        "cargo:rustc-env=GLIDE_APPLE_HELPER_PATH={}",
        helper.display()
    );
}

fn maybe_codesign_helper(helper: &Path) {
    let Some(identity) = codesign_identity() else {
        return;
    };

    let status = Command::new("codesign")
        .args([
            "--force",
            "--timestamp=none",
            "--sign",
            &identity,
            "--identifier",
            "com.glide.app.apple-helper",
            helper.to_str().unwrap(),
        ])
        .status()
        .expect("failed to invoke codesign for GlideAppleHelper");

    if !status.success() {
        panic!("failed to codesign GlideAppleHelper");
    }
}

fn codesign_identity() -> Option<String> {
    env_nonempty("GLIDE_APPLE_CODESIGN_IDENTITY")
        .or_else(|| env_nonempty("APPLE_CODESIGN_IDENTITY"))
        .or_else(|| {
            team_id()
                .as_deref()
                .and_then(find_codesign_identity_for_team)
        })
}

fn team_id() -> Option<String> {
    env_nonempty("GLIDE_APPLE_TEAM_ID")
        .or_else(|| env_nonempty("APPLE_TEAM_ID"))
        .or_else(|| Some(DEFAULT_APPLE_TEAM_ID.to_string()))
}

fn env_nonempty(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn find_codesign_identity_for_team(team_id: &str) -> Option<String> {
    let output = Command::new("security")
        .args(["find-identity", "-v", "-p", "codesigning"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let team_marker = format!("({team_id})");
    text.lines()
        .find(|line| line.contains(&team_marker) || line.contains(team_id))
        .and_then(identity_hash)
}

fn identity_hash(line: &str) -> Option<String> {
    line.split_whitespace()
        .nth(1)
        .map(str::to_string)
        .filter(|identity| !identity.trim().is_empty())
}
