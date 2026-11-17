//! Names: turning what an archive says a file is called into a path that is
//! safe to write, readable in any language, and does not clobber anything.

use std::path::{Path, PathBuf};

/// Decode a file name stored as raw bytes (zip, tar). Modern archives use
/// UTF-8. Older ones use whatever the making computer used: Shift_JIS on a
/// Japanese PC, GBK in China, CP437 on an English one. Guess from the bytes,
/// because the archive does not say.
pub fn decode_name(raw: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(raw) {
        return s.to_string();
    }
    let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Deny);
    detector.feed(raw, true);
    let encoding = detector.guess(None, chardetng::Utf8Detection::Deny);
    // Western names with accents come from DOS-era code pages, which the
    // zip format defaults to, rather than from Windows-1252.
    if encoding != encoding_rs::WINDOWS_1252 {
        let (text, _, had_errors) = encoding.decode(raw);
        if !had_errors {
            return text.into_owned();
        }
    }
    cp437(raw)
}

/// IBM code page 437, the zip format's historical default.
fn cp437(raw: &[u8]) -> String {
    const HIGH: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u{a0}";
    let high: Vec<char> = HIGH.chars().collect();
    raw.iter()
        .map(|&b| {
            if b < 0x80 {
                b as char
            } else {
                high[(b - 0x80) as usize]
            }
        })
        .collect()
}

/// The relative path an entry may be written to, or `None` if it must not be
/// written at all.
///
/// Archives can name `/etc/passwd` or `../../.zshrc` ("zip slip"). This
/// keeps every entry inside the destination: absolute paths and drive
/// letters become relative, `.` is dropped, and `..` is resolved, refusing
/// any name that climbs out. Windows-made archives that use `\` are read
/// as folders.
pub fn safe_relative(name: &str) -> Option<PathBuf> {
    if name.contains('\0') {
        return None;
    }
    let unified = name.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for (i, part) in unified.split('/').enumerate() {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            // "C:" at the start of a Windows path.
            p if i == 0 && p.len() == 2 && p.ends_with(':') => {}
            p => parts.push(p),
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.iter().collect())
}

/// The path as the interface shows and selects it: always `/`-separated.
pub fn display_path(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Files that only one operating system cares about, and that clutter a
/// folder on every other: Finder's resource forks and window settings, and
/// Windows' thumbnail caches.
pub fn is_junk(path: &Path) -> bool {
    path.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s == "__MACOSX"
            || s == ".DS_Store"
            || s.starts_with("._")
            || s == "Thumbs.db"
            || s == "desktop.ini"
    })
}

/// Suffixes that belong to the archive, not to the name of what is inside,
/// longest first so `.tar.gz` wins over `.gz`.
const SUFFIXES: &[&str] = &[
    ".tar.gz",
    ".tar.bz2",
    ".tar.xz",
    ".tar.zst",
    ".tar.zstd",
    ".tgz",
    ".tbz2",
    ".tbz",
    ".txz",
    ".tzst",
    ".zip",
    ".zipx",
    ".rar",
    ".7z",
    ".tar",
    ".gz",
    ".bz2",
    ".xz",
    ".zst",
    ".zstd",
    ".cbz",
    ".cbr",
    ".cb7",
    ".jar",
];

/// The name for the folder an archive unpacks into: `Photos.part1.rar`,
/// `Photos.7z.001` and `Photos.tar.gz` all give `Photos`.
pub fn archive_stem(path: &Path) -> String {
    let mut name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    // Volume numbers: .001, .part01 (before .rar), .r00 is handled by the rar suffix being absent.
    if let Some((head, tail)) = name.rsplit_once('.') {
        if tail.len() == 3 && tail.chars().all(|c| c.is_ascii_digit()) {
            name = head.to_string();
        }
    }
    let lower = name.to_lowercase();
    for suffix in SUFFIXES {
        if lower.ends_with(suffix) && lower.len() > suffix.len() {
            name.truncate(name.len() - suffix.len());
            break;
        }
    }
    if let Some(i) = name.to_lowercase().rfind(".part") {
        if name[i + 5..].chars().all(|c| c.is_ascii_digit()) && i > 0 && name.len() > i + 5 {
            name.truncate(i);
        }
    }
    if name.is_empty() {
        "Archive".into()
    } else {
        name
    }
}

/// A path in `dir` named `name` that does not exist yet: `name`, then
/// `name 2`, `name 3`… keeping any extension, the way Finder does. Nothing
/// that is already there is ever replaced.
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !exists(&first) {
        return first;
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (2..)
        .map(|n| dir.join(format!("{stem} {n}{ext}")))
        .find(|p| !exists(p))
        .expect("an unused name")
}

/// True for anything at the path, including a broken symlink.
fn exists(p: &Path) -> bool {
    p.symlink_metadata().is_ok()
}

/// A size for people: "1.2 GB", "340 KB". Decimal units, as Finder uses.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["bytes", "KB", "MB", "GB", "TB"];
    if bytes < 1000 {
        return format!("{bytes} bytes");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if value < 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_ordinary_paths() {
        assert_eq!(safe_relative("a/b/c.txt"), Some(PathBuf::from("a/b/c.txt")));
        assert_eq!(safe_relative("./a//b/"), Some(PathBuf::from("a/b")));
    }

    #[test]
    fn refuses_to_climb_out() {
        assert_eq!(safe_relative("../evil"), None);
        assert_eq!(safe_relative("a/../../evil"), None);
        assert_eq!(safe_relative("a/../b"), Some(PathBuf::from("b")));
    }

    #[test]
    fn absolute_and_windows_paths_become_relative() {
        assert_eq!(
            safe_relative("/etc/passwd"),
            Some(PathBuf::from("etc/passwd"))
        );
        assert_eq!(
            safe_relative("C:\\Users\\a.txt"),
            Some(PathBuf::from("Users/a.txt"))
        );
        assert_eq!(
            safe_relative("dir\\file.txt"),
            Some(PathBuf::from("dir/file.txt"))
        );
    }

    #[test]
    fn empty_and_nul_names_are_refused() {
        assert_eq!(safe_relative(""), None);
        assert_eq!(safe_relative("/"), None);
        assert_eq!(safe_relative("a\0b"), None);
    }

    #[test]
    fn decodes_japanese_and_legacy_names() {
        assert_eq!(decode_name("写真.jpg".as_bytes()), "写真.jpg");
        let (sjis, _, _) = encoding_rs::SHIFT_JIS.encode("日本語のファイル名です.txt");
        assert_eq!(decode_name(&sjis), "日本語のファイル名です.txt");
        assert_eq!(cp437(&[0x80, b'a']), "Ça");
    }

    #[test]
    fn spots_mac_and_windows_clutter() {
        assert!(is_junk(Path::new("__MACOSX/a/._b")));
        assert!(is_junk(Path::new("photos/.DS_Store")));
        assert!(is_junk(Path::new("Thumbs.db")));
        assert!(!is_junk(Path::new("photos/a.jpg")));
    }

    #[test]
    fn stems_drop_archive_suffixes_and_volume_numbers() {
        assert_eq!(archive_stem(Path::new("/x/Photos.tar.gz")), "Photos");
        assert_eq!(archive_stem(Path::new("Photos.part1.rar")), "Photos");
        assert_eq!(archive_stem(Path::new("Photos.7z.001")), "Photos");
        assert_eq!(archive_stem(Path::new("v1.2.zip")), "v1.2");
        assert_eq!(archive_stem(Path::new(".zip")), ".zip");
    }

    #[test]
    fn unique_paths_never_replace() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(unique_path(dir.path(), "a.txt"), dir.path().join("a.txt"));
        std::fs::write(dir.path().join("a.txt"), "").unwrap();
        std::fs::write(dir.path().join("a 2.txt"), "").unwrap();
        assert_eq!(unique_path(dir.path(), "a.txt"), dir.path().join("a 3.txt"));
        std::fs::create_dir(dir.path().join("Folder")).unwrap();
        assert_eq!(
            unique_path(dir.path(), "Folder"),
            dir.path().join("Folder 2")
        );
    }

    #[test]
    fn sizes_read_like_finder() {
        assert_eq!(human_size(12), "12 bytes");
        assert_eq!(human_size(1_234_567), "1.2 MB");
        assert_eq!(human_size(340_000), "340 KB");
    }
}
