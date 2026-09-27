pub mod names;
pub mod sprites;

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub struct SpriteId(pub usize);
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub struct SoundId(pub usize);

/// The one file a release build keeps every resource in; build.rs packs it from Resources.
const ARCHIVE: &str = "Resources.tar";
/// The unpacked resources of the source tree, which debug builds read.
const SOURCE_FOLDER: &str = "Resources";

/// The folders inside the resources.
pub const TEXTURES: &str = "Textures";
pub const SOUNDS: &str = "Sounds";
pub const FONTS: &str = "Fonts";
pub const MAPS: &str = "Maps";

/// A release build is portable: the folder the program lies in becomes its working
/// folder, so Resources.tar is looked for, and PersistentData and logs are written,
/// beside it however it was started (a shortcut, another folder's terminal). False
/// when Resources.tar is not there.
#[cfg(not(debug_assertions))]
pub fn enter_program_folder() -> bool {
    let folder = std::env::current_exe().ok().and_then(|program| program.parent().map(Path::to_path_buf));
    if let Some(folder) = folder {
        if let Err(error) = std::env::set_current_dir(&folder) {
            eprintln!("could not work from {}: {error}", folder.display());
        }
    }
    Path::new(ARCHIVE).is_file()
}

/// Where Textures/, Sounds/, Fonts/, Maps/ and Icon/ are: inside Resources.tar in a
/// release, the Resources folder in the source tree and in debug builds. Paths are relative
/// to the working directory, like the server's own files. A path under the archive is
/// no real folder, so resource files are read with the functions below, not std::fs.
pub fn content_folder() -> PathBuf {
    if archive().is_some() {
        PathBuf::from(ARCHIVE)
    } else {
        PathBuf::from(SOURCE_FOLDER)
    }
}

/// The bytes of a resource file, from the archive or from the disk.
pub fn read(path: &Path) -> Result<Vec<u8>> {
    let (Some(archive), Some(inside)) = (archive(), path_inside_archive(path)) else {
        return std::fs::read(path).with_context(|| format!("reading {}", path.display()));
    };
    archive.read(&inside)
}

pub fn read_to_string(path: &Path) -> Result<String> {
    String::from_utf8(read(path)?).with_context(|| format!("{} is not text", path.display()))
}

pub fn exists(path: &Path) -> bool {
    match (archive(), path_inside_archive(path)) {
        (Some(archive), Some(inside)) => archive.files.contains_key(&inside),
        _ => path.exists(),
    }
}

/// Every file under `folder` and its subfolders with one of `extensions`, by file
/// name without extension. Packs may sort files into any folders they like.
pub fn find_files(folder: &Path, extensions: &[&str]) -> Result<HashMap<String, PathBuf>> {
    let mut by_name = HashMap::new();
    match (archive(), path_inside_archive(folder)) {
        (Some(archive), Some(inside)) => {
            for file in archive.files.keys().filter(|file| file.starts_with(&inside)) {
                add_if_wanted(Path::new(ARCHIVE).join(file), extensions, &mut by_name);
            }
        }
        _ => collect_files(folder, extensions, &mut by_name)?,
    }
    Ok(by_name)
}

fn collect_files(folder: &Path, extensions: &[&str], by_name: &mut HashMap<String, PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(folder).with_context(|| format!("reading {}", folder.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            collect_files(&path, extensions, by_name)?;
        } else {
            add_if_wanted(path, extensions, by_name);
        }
    }
    Ok(())
}

fn add_if_wanted(path: PathBuf, extensions: &[&str], by_name: &mut HashMap<String, PathBuf>) {
    if path.extension().is_some_and(|extension| extensions.iter().any(|wanted| extension == *wanted)) {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        by_name.insert(name, path);
    }
}

/// Resources.tar, opened once: the file, and where each file's bytes are in it.
struct Archive {
    file: Mutex<File>,
    /// Start and length of each file's bytes, by its path inside the archive.
    files: HashMap<PathBuf, (u64, u64)>,
}

impl Archive {
    fn read(&self, inside: &Path) -> Result<Vec<u8>> {
        let &(start, size) = self.files.get(inside).with_context(|| format!("{} is not in {ARCHIVE}", inside.display()))?;
        let mut bytes = vec![0; size as usize];
        let mut file = self.file.lock().unwrap_or_else(PoisonError::into_inner);
        file.seek(SeekFrom::Start(start))?;
        file.read_exact(&mut bytes).with_context(|| format!("reading {} from {ARCHIVE}", inside.display()))?;
        Ok(bytes)
    }
}

/// The archive, or None when there is none beside the game (the source tree, debug builds).
fn archive() -> Option<&'static Archive> {
    static OPENED: OnceLock<Option<Archive>> = OnceLock::new();
    OPENED
        .get_or_init(|| {
            let file = File::open(ARCHIVE).ok()?;
            // The game cannot run without its resources, and a broken archive has no fallback.
            Some(index_archive(file).unwrap_or_else(|error| panic!("{ARCHIVE} is broken: {error:#}")))
        })
        .as_ref()
}

/// Reads only the headers: the tar reader seeks past the bytes of each file.
fn index_archive(file: File) -> Result<Archive> {
    let mut files = HashMap::new();
    for entry in tar::Archive::new(&file).entries_with_seek()? {
        let entry = entry?;
        if entry.header().entry_type().is_file() {
            files.insert(entry.path()?.into_owned(), (entry.raw_file_position(), entry.size()));
        }
    }
    Ok(Archive { file: Mutex::new(file), files })
}

/// The path inside the archive of `path` under Resources.tar. Maps name their
/// tilesets and images relative to themselves ("../Textures/..."), and a folder
/// would resolve that "..", so the archive does the same.
fn path_inside_archive(path: &Path) -> Option<PathBuf> {
    let mut inside = PathBuf::new();
    for part in path.strip_prefix(ARCHIVE).ok()?.components() {
        match part {
            Component::ParentDir => {
                inside.pop();
            }
            Component::CurDir => {}
            other => inside.push(other),
        }
    }
    Some(inside)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_gives_back_each_file() {
        let folder = std::env::temp_dir().join(format!("core_resources_{}", std::process::id()));
        std::fs::create_dir_all(folder.join("Maps")).unwrap();
        std::fs::write(folder.join("Maps/level.tmj"), "{}").unwrap();
        std::fs::write(folder.join("Maps/tiles.tsj"), "[1, 2]").unwrap();
        let tar_path = folder.with_extension("tar");
        let mut builder = tar::Builder::new(File::create(&tar_path).unwrap());
        builder.append_dir_all("", &folder).unwrap();
        builder.finish().unwrap();

        let archive = index_archive(File::open(&tar_path).unwrap()).unwrap();
        assert_eq!(archive.read(Path::new("Maps/tiles.tsj")).unwrap(), b"[1, 2]");
        assert_eq!(archive.read(Path::new("Maps/level.tmj")).unwrap(), b"{}");
        assert!(archive.read(Path::new("Maps/missing.tmj")).is_err());
        std::fs::remove_dir_all(&folder).unwrap();
        std::fs::remove_file(&tar_path).unwrap();
    }

    #[test]
    fn every_named_sprite_has_a_file() {
        let files = find_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join(SOURCE_FOLDER).join(TEXTURES), &["png", "apng"]).unwrap();
        for name in names::SPRITE_FILES {
            assert!(files.contains_key(name), "{name}.png (or .apng) is not in {TEXTURES}");
        }
    }

    #[test]
    fn paths_relative_to_a_map_resolve_inside_the_archive() {
        let tileset_image = Path::new("Resources.tar/Maps/Tilesets/../../Textures/./Zones/test.apng");
        assert_eq!(path_inside_archive(tileset_image), Some(PathBuf::from("Textures/Zones/test.apng")));
        assert_eq!(path_inside_archive(Path::new("Resources/Maps/level1.tmj")), None);
    }
}
