use std::{env, path::PathBuf, process::Command};

fn xcrun(args: &[&str]) -> String {
    let output = Command::new("xcrun").args(args).output().unwrap_or_else(|e| {
        panic!("Cannot run xcrun: {e}. Select an Apple toolchain with xcode-select; macOS 26+ SDK and Swift 6 are required.")
    });
    if !output.status.success() {
        panic!("xcrun {} failed: {}. Select an Apple toolchain containing the macOS 26+ SDK and FoundationModels.framework.", args.join(" "), String::from_utf8_lossy(&output.stderr));
    }
    String::from_utf8(output.stdout)
        .expect("xcrun output must be UTF-8")
        .trim()
        .to_owned()
}

fn main() {
    println!("cargo:rerun-if-changed=swift/MikomaiFoundationModels.swift");
    println!("cargo:rerun-if-env-changed=DEVELOPER_DIR");
    println!("cargo:rerun-if-env-changed=SDKROOT");
    println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    if env::consts::OS != "macos" {
        panic!("mikomai-llm-apple must be built on macOS with an Apple Swift toolchain.");
    }

    let sdk = PathBuf::from(xcrun(&["--sdk", "macosx", "--show-sdk-path"]));
    let frameworks = sdk.join("System/Library/Frameworks");
    if !frameworks
        .join("FoundationModels.framework/Modules")
        .is_dir()
    {
        panic!("FoundationModels.framework is missing from {}. Install/select an Apple toolchain with the macOS 26+ SDK using xcode-select or DEVELOPER_DIR.", sdk.display());
    }
    let arch = match env::var("CARGO_CFG_TARGET_ARCH").unwrap().as_str() {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        other => panic!("Unsupported macOS architecture for Foundation Models: {other}"),
    };
    // The native library requires macOS 26. Its own load commands select the
    // system Swift concurrency runtime even if the Rust host targets older OSes.
    let deployment = env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "26.0".into());
    if deployment
        .split('.')
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0)
        < 26
    {
        panic!("mikomai-llm-apple requires MACOSX_DEPLOYMENT_TARGET=26.0 or later");
    }
    let target = format!("{arch}-apple-macosx{deployment}");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let library = out.join("libMikomaiFoundationModels.dylib");
    let output = Command::new("xcrun")
        .args([
            "--sdk",
            "macosx",
            "swiftc",
            "-swift-version",
            "6",
            "-parse-as-library",
            "-emit-library",
            "-module-name",
            "MikomaiFoundationModels",
            "-target",
            &target,
            "-sdk",
        ])
        .arg(&sdk)
        .arg("swift/MikomaiFoundationModels.swift")
        .arg("-o")
        .arg(&library)
        // Keep Cargo-run binaries and downstream crates independent of the
        // caller's rpath flags. Distributors must ship this dylib and relocate
        // its install name to their app's Frameworks directory.
        .args(["-Xlinker", "-install_name", "-Xlinker"])
        .arg(&library)
        // Rust's sandbox can leave the default Clang module cache unwritable.
        .arg("-module-cache-path")
        .arg(out.join("swift-module-cache"))
        .output()
        .expect("Cannot start Swift compiler via xcrun");
    if !output.status.success() {
        panic!("Cannot compile the Foundation Models Swift bridge. A macOS 26+ SDK with FoundationModels and Swift 6 is required:\n{}", String::from_utf8_lossy(&output.stderr));
    }
    // swiftc links the dylib's Foundation/Swift runtime dependencies using the
    // active toolchain and SDK. Rust must not relink swift_Concurrency against
    // an older host deployment target (which changes its install name to @rpath).
    println!("cargo:rustc-link-search=framework={}", frameworks.display());
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=dylib=MikomaiFoundationModels");
    println!("cargo:rustc-link-lib=framework=FoundationModels");
    println!("cargo:rustc-link-lib=framework=Foundation");
}
