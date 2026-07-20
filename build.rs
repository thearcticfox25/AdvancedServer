use std::{env, path::Path, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=LICENSE.txt");
    println!("cargo:rerun-if-changed=build.rs");

    if !Path::new("LICENSE.txt").exists() {
        panic!(
            "\n\n\
             ======================================================\n\
             ERROR: LICENSE.txt not found!\n\
             This project is licensed under GNU AGPL-3.0.\n\
             LICENSE.txt must be present in the project root.\n\
             You cannot legally build or distribute this software\n\
             without including the license.\n\
             ======================================================\n"
        );
    }

    // Build metadata for the startup banner
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| "unknown".into());
    println!("cargo:rustc-env=BUILD_TARGET_OS={}", target_os);

    let rustc_bin = env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let rustc_ver = Command::new(&rustc_bin)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| {
            let t = s.trim();
            // "rustc 1.95.0 (hash date)" -> "rustc 1.95.0"
            let mut parts = t.splitn(3, ' ');
            match (parts.next(), parts.next()) {
                (Some(a), Some(b)) => format!("{} {}", a, b),
                _ => t.to_string(),
            }
        })
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=BUILD_RUSTC_VERSION={}", rustc_ver);

    let build_date = Command::new("date")
        .arg("+%b %d %Y")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    let build_time = Command::new("date")
        .arg("+%H:%M:%S")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=BUILD_DATE={}", build_date);
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);
}
