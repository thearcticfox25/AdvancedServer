use std::{env, fs, io, path::Path, process::Command};

/// The game's resources in the source tree, and the one file a release build packs
/// them into (core/resources reads either).
const SOURCE_FOLDER: &str = "Resources";
const ARCHIVE: &str = "Resources.tar";

fn main() {
    println!("cargo:rerun-if-changed=LICENSE.txt");
    println!("cargo:rerun-if-changed=build.rs");
    // A directory: cargo reruns this when any file inside it changes.
    println!("cargo:rerun-if-changed={SOURCE_FOLDER}");

    // The game looks for its resources in the folder it runs from, so the built binary gets
    // them beside it. OUT_DIR is target/<profile>/build/<crate>/out. A debug build gets a
    // copy of the Resources folder, a release build one Resources.tar (core/resources reads
    // either): one open file instead of a file system lookup for every resource.
    let out_dir = env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    if let Some(profile_dir) = Path::new(&out_dir).ancestors().nth(3) {
        if env::var("PROFILE").as_deref() == Ok("release") {
            pack(Path::new(SOURCE_FOLDER), &profile_dir.join(ARCHIVE)).expect("packing Resources into Resources.tar");
            // A copy left from an older build would only confuse whoever looks there.
            let old_copy = profile_dir.join(SOURCE_FOLDER);
            if old_copy.is_dir() {
                fs::remove_dir_all(old_copy).expect("removing the old Resources copy");
            }
        } else {
            sync(Path::new(SOURCE_FOLDER), &profile_dir.join(SOURCE_FOLDER)).expect("copying Resources beside the binary");
        }
    }

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

/// Makes `to` a copy of `from`: new and changed files are copied, files gone from
/// `from` are removed. Unchanged files (same size, not newer) are left alone, so only
/// the first build copies everything.
fn sync(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let (source, target) = (entry.path(), to.join(entry.file_name()));
        if entry.file_type()?.is_dir() {
            sync(&source, &target)?;
            continue;
        }
        let meta = entry.metadata()?;
        let unchanged = fs::metadata(&target).is_ok_and(|copied| copied.len() == meta.len() && copied.modified().ok() >= meta.modified().ok());
        if !unchanged {
            fs::copy(&source, &target)?;
        }
    }
    for entry in fs::read_dir(to)? {
        let entry = entry?;
        if !from.join(entry.file_name()).exists() {
            if entry.file_type()?.is_dir() { fs::remove_dir_all(entry.path())? } else { fs::remove_file(entry.path())? }
        }
    }
    Ok(())
}

/// Writes every file of `from` into the archive `to`, by its path inside `from`. Cargo
/// runs this only when something in `from` changed, and then every file may have, so
/// the archive is written anew; it goes to a temporary name first, so a build stopped
/// halfway never leaves a broken archive for the game to read.
fn pack(from: &Path, to: &Path) -> io::Result<()> {
    let unfinished = to.with_extension("tar.part");
    let mut archive = tar::Builder::new(io::BufWriter::new(fs::File::create(&unfinished)?));
    archive.append_dir_all("", from)?;
    archive.into_inner()?.into_inner()?;
    fs::rename(unfinished, to)
}
