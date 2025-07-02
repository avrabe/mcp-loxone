//! Build script for WASM32-WASIP2 compilation optimizations and macOS code signing

use std::env;
use std::process::Command;

fn main() {
    let target = env::var("TARGET").unwrap_or_default();

    // macOS code signing setup
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "macos" {
        setup_macos_codesigning();
    }

    // Configure build for WASM targets
    if target.starts_with("wasm32") {
        println!("cargo:rustc-cfg=target_family=\"wasm\"");

        // WASI-specific configuration
        if target == "wasm32-wasip2" {
            println!("cargo:rustc-cfg=wasi");
            println!("cargo:rustc-cfg=wasip2");
        }

        // Optimize for size in WASM builds
        println!("cargo:rustc-link-arg=--gc-sections");
        println!("cargo:rustc-link-arg=--strip-all");

        // Enable specific features for WASM
        println!("cargo:rustc-cfg=feature=\"wasm-size-optimized\"");
    }

    // Native-specific configuration
    if !target.starts_with("wasm32") {
        println!("cargo:rustc-cfg=native");

        // Enable keyring only on native targets
        if cfg!(feature = "keyring-storage") {
            println!("cargo:rustc-cfg=feature=\"keyring\"");
        }
    }

    // Feature detection
    detect_features();

    // Version information
    set_version_info();
}

fn detect_features() {
    // Detect if we're building with specific features
    let features = ["crypto", "websocket", "keyring-storage", "wasm-storage"];

    for feature in &features {
        if env::var(format!(
            "CARGO_FEATURE_{}",
            feature.to_uppercase().replace('-', "_")
        ))
        .is_ok()
        {
            println!("cargo:rustc-cfg=has_feature=\"{feature}\"");
        }
    }
}

fn set_version_info() {
    // Pass version information to the build
    if let Ok(version) = env::var("CARGO_PKG_VERSION") {
        println!("cargo:rustc-env=BUILD_VERSION={version}");
    }

    if let Ok(git_hash) = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        if git_hash.status.success() {
            let hash = String::from_utf8_lossy(&git_hash.stdout);
            println!("cargo:rustc-env=BUILD_GIT_HASH={}", hash.trim());
        }
    }

    // Build timestamp
    println!(
        "cargo:rustc-env=BUILD_TIMESTAMP={}",
        chrono::Utc::now().to_rfc3339()
    );
}

fn setup_macos_codesigning() {
    // Tell Cargo to re-run this script if these change
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CODESIGN_IDENTITY");
    println!("cargo:rerun-if-env-changed=SKIP_CODESIGN");

    // Skip signing if explicitly requested
    if env::var("SKIP_CODESIGN").is_ok() {
        println!("cargo:warning=Skipping code signing (SKIP_CODESIGN set)");
        return;
    }

    // Create a post-build script that can be run after cargo build
    create_post_build_script();

    // Set an environment variable to indicate code signing is available
    println!("cargo:rustc-env=MACOS_CODESIGN_AVAILABLE=1");

    // Provide instructions to the user
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "macos" {
        println!("cargo:warning=macOS detected: Use './cargo-build-sign.sh' or 'make build' for automatic code signing");
    }
}

fn create_post_build_script() {
    // Create a post-build script in the project root
    let script_content = r#"#!/bin/bash
# Post-build code signing script for macOS
# Run this after 'cargo build' to sign the binary

set -e

# Find the most recently built binary
BINARY_PATH=""
if [ -f "target/release/loxone-mcp-server" ]; then
    BINARY_PATH="target/release/loxone-mcp-server"
elif [ -f "target/debug/loxone-mcp-server" ]; then
    BINARY_PATH="target/debug/loxone-mcp-server"
fi

if [ -z "$BINARY_PATH" ]; then
    echo "❌ No binary found to sign"
    exit 1
fi

echo "🔐 Signing $BINARY_PATH..."

# Check if already signed
if codesign --verify "$BINARY_PATH" 2>/dev/null; then
    echo "✅ Binary is already signed"
    exit 0
fi

# Sign the binary
if [ -n "$CODESIGN_IDENTITY" ]; then
    codesign -s "$CODESIGN_IDENTITY" --force --deep --preserve-metadata=entitlements "$BINARY_PATH"
else
    # Ad-hoc signing for development
    codesign -s - --force --deep "$BINARY_PATH"
fi

if [ $? -eq 0 ]; then
    echo "✅ Code signing successful"
    codesign --verify --verbose=2 "$BINARY_PATH"
else
    echo "❌ Code signing failed"
    exit 1
fi
"#;

    if std::fs::write("post-build-sign.sh", script_content).is_ok() {
        Command::new("chmod")
            .arg("+x")
            .arg("post-build-sign.sh")
            .output()
            .ok();

        println!(
            "cargo:warning=Created post-build-sign.sh - run this after building to sign the binary"
        );
    }
}
