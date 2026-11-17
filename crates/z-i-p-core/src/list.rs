//! Looking inside an archive without unpacking it, so a person can see what
//! they are about to get and pick only the files they want.

use std::path::Path;

use serde::Serialize;

use crate::error::{Error, Result};
use crate::format::{decompressor, detect, Format, Source};
use crate::names::{archive_stem, decode_name, display_path, safe_relative};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// `/`-separated path inside the archive, already made safe.
    pub path: String,
    pub is_dir: bool,
    /// Unpacked size, when the archive records it.
    pub size: Option<u64>,
    pub encrypted: bool,
    /// Seconds since 1970, when recorded.
    pub modified: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub format: Format,
    pub format_label: String,
    /// The archive's name without its suffixes: the folder it unpacks into.
    pub stem: String,
    pub entries: Vec<Entry>,
    /// Sum of the entries' sizes, if every size is known.
    pub total_size: Option<u64>,
    /// Size of the archive itself, all pieces together.
    pub archive_size: u64,
    pub encrypted: bool,
    pub pieces: usize,
    /// Entries left out because their names would escape the destination.
    pub unsafe_names: usize,
}

pub fn list(path: &Path, password: Option<&str>) -> Result<Listing> {
    let source = Source::for_path(path)?;
    let format = detect(&source)?;
    let mut unsafe_names = 0;
    let entries = match format {
        Format::Zip => list_zip(&source, &mut unsafe_names)?,
        Format::SevenZ => list_7z(&source, password, &mut unsafe_names)?,
        Format::Rar => list_rar(source.first(), password, &mut unsafe_names)?,
        f if f.is_single_stream() => vec![Entry {
            path: single_stream_name(source.first()),
            is_dir: false,
            size: None,
            encrypted: false,
            modified: None,
        }],
        _ => list_tar(&source, format, &mut unsafe_names)?,
    };
    let total_size = entries
        .iter()
        .filter(|e| !e.is_dir)
        .map(|e| e.size)
        .sum::<Option<u64>>();
    Ok(Listing {
        format,
        format_label: format.label().into(),
        stem: archive_stem(source.first()),
        encrypted: entries.iter().any(|e| e.encrypted),
        total_size,
        archive_size: source.len(),
        pieces: source.piece_count(),
        unsafe_names,
        entries,
    })
}

/// The file a `.gz`/`.bz2`/`.xz`/`.zst` holds is named after it, minus
/// the suffix: `notes.txt.gz` holds `notes.txt`.
pub(crate) fn single_stream_name(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    for suffix in [".gz", ".bz2", ".xz", ".zst", ".zstd", ".z"] {
        if name.to_lowercase().ends_with(suffix) && name.len() > suffix.len() {
            return name[..name.len() - suffix.len()].to_string();
        }
    }
    format!("{name} (unpacked)")
}

fn safe(name: &str, unsafe_names: &mut usize) -> Option<String> {
    let p = safe_relative(name);
    if p.is_none() && !name.trim_matches(['/', '\\', '.']).is_empty() {
        *unsafe_names += 1;
    }
    p.map(|p| display_path(&p))
}

fn list_zip(source: &Source, unsafe_names: &mut usize) -> Result<Vec<Entry>> {
    let mut zip = zip::ZipArchive::new(source.open()?).map_err(map_zip)?;
    let mut out = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let f = zip.by_index_raw(i).map_err(map_zip)?;
        let Some(path) = safe(&decode_name(f.name_raw()), unsafe_names) else {
            continue;
        };
        out.push(Entry {
            path,
            is_dir: f.is_dir(),
            size: Some(f.size()),
            encrypted: f.encrypted(),
            modified: f.last_modified().map(|d| {
                unix_seconds(
                    d.year() as i64,
                    d.month() as i64,
                    d.day() as i64,
                    d.hour(),
                    d.minute(),
                    d.second(),
                )
            }),
        });
    }
    Ok(out)
}

fn list_7z(
    source: &Source,
    password: Option<&str>,
    unsafe_names: &mut usize,
) -> Result<Vec<Entry>> {
    let pw = password
        .map(sevenz_rust2::Password::from)
        .unwrap_or_else(sevenz_rust2::Password::empty);
    let archive = sevenz_rust2::Archive::read(&mut source.open()?, &pw)
        .map_err(|e| map_7z(e, password.is_some()))?;
    let mut out = Vec::with_capacity(archive.files.len());
    for (i, f) in archive.files.iter().enumerate() {
        if f.is_anti_item() {
            continue;
        }
        let Some(path) = safe(f.name(), unsafe_names) else {
            continue;
        };
        let modified = f.has_last_modified_date.then(|| {
            std::time::SystemTime::from(f.last_modified_date())
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        });
        out.push(Entry {
            path,
            is_dir: f.is_directory(),
            size: Some(f.size()),
            encrypted: archive.is_file_encrypted(i),
            modified,
        });
    }
    Ok(out)
}

fn list_rar(path: &Path, password: Option<&str>, unsafe_names: &mut usize) -> Result<Vec<Entry>> {
    let archive = match password {
        Some(pw) => unrar::Archive::with_password(path, pw.as_bytes()),
        None => unrar::Archive::new(path),
    };
    let archive = if archive.is_multipart() {
        archive.as_first_part()
    } else {
        archive
    };
    let mut cursor = archive.open_for_listing().map_err(map_rar)?;
    let mut out = Vec::new();
    while let Some(header) = cursor.read_header().map_err(map_rar)? {
        let e = header.entry();
        if let Some(path) = safe(&e.filename.to_string_lossy(), unsafe_names) {
            out.push(Entry {
                path,
                is_dir: e.is_directory(),
                size: Some(e.unpacked_size),
                encrypted: e.is_encrypted(),
                modified: Some(dos_time(e.file_time)),
            });
        }
        cursor = header.skip().map_err(map_rar)?;
    }
    Ok(out)
}

fn list_tar(source: &Source, format: Format, unsafe_names: &mut usize) -> Result<Vec<Entry>> {
    let reader = decompressor(format, source.open()?)?;
    let mut tar = tar::Archive::new(reader);
    let mut out = Vec::new();
    for entry in tar.entries()? {
        let entry = entry?;
        let header = entry.header();
        let kind = header.entry_type();
        if !(kind.is_file() || kind.is_dir() || kind.is_symlink() || kind.is_hard_link()) {
            continue;
        }
        let Some(path) = safe(&decode_name(&entry.path_bytes()), unsafe_names) else {
            continue;
        };
        out.push(Entry {
            path,
            is_dir: kind.is_dir(),
            size: Some(entry.size()),
            encrypted: false,
            modified: header.mtime().ok().map(|t| t as i64),
        });
    }
    Ok(out)
}

/// Seconds since 1970 for a calendar date and time (proleptic Gregorian).
pub(crate) fn unix_seconds(y: i64, m: i64, d: i64, hh: u8, mm: u8, ss: u8) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    days * 86_400 + hh as i64 * 3600 + mm as i64 * 60 + ss as i64
}

/// An MS-DOS packed date and time, as RAR stores it.
fn dos_time(t: u32) -> i64 {
    let date = t >> 16;
    let time = t & 0xFFFF;
    unix_seconds(
        1980 + (date >> 9) as i64,
        ((date >> 5) & 0xF).max(1) as i64,
        (date & 0x1F).max(1) as i64,
        (time >> 11) as u8,
        ((time >> 5) & 0x3F) as u8,
        ((time & 0x1F) * 2) as u8,
    )
}

pub(crate) fn map_zip(e: zip::result::ZipError) -> Error {
    use zip::result::ZipError as Z;
    match e {
        Z::Io(io) => io.into(),
        Z::InvalidPassword => Error::WrongPassword,
        Z::UnsupportedArchive(msg) if msg.to_lowercase().contains("password") => {
            Error::PasswordRequired
        }
        Z::UnsupportedArchive(msg) => Error::Unsupported {
            detail: msg.to_string(),
        },
        Z::CompressionMethodNotSupported(m) => Error::Unsupported {
            detail: format!("zip compression method {m}"),
        },
        Z::InvalidArchive(msg) => Error::Damaged {
            detail: msg.to_string(),
        },
        other => Error::Damaged {
            detail: other.to_string(),
        },
    }
}

pub(crate) fn map_7z(e: sevenz_rust2::Error, had_password: bool) -> Error {
    use sevenz_rust2::Error as S;
    match e {
        S::PasswordRequired => Error::PasswordRequired,
        S::MaybeBadPassword(_) => Error::WrongPassword,
        S::Io(io, _) | S::FileOpen(io, _) => {
            if had_password && io.kind() == std::io::ErrorKind::InvalidData {
                Error::WrongPassword
            } else {
                io.into()
            }
        }
        S::ChecksumVerificationFailed if had_password => Error::WrongPassword,
        S::UnsupportedCompressionMethod(m) => Error::Unsupported {
            detail: format!("7z method {m}"),
        },
        S::Unsupported(m) => Error::Unsupported {
            detail: m.to_string(),
        },
        S::MaxMemLimited { .. } => Error::Unsupported {
            detail: "this archive needs more memory to unpack than is safe.".into(),
        },
        other => Error::Damaged {
            detail: other.to_string(),
        },
    }
}

pub(crate) fn map_rar(e: unrar::error::UnrarError) -> Error {
    use unrar::error::Code as C;
    match e.code {
        C::MissingPassword => Error::PasswordRequired,
        C::BadPassword => Error::WrongPassword,
        C::EOpen => Error::MissingPart {
            detail: "Put all the parts (.part1.rar, .part2.rar…) in the same folder.".into(),
        },
        C::BadData | C::BadArchive | C::EndArchive => Error::Damaged {
            detail: "If it was downloaded, the download may not have finished.".into(),
        },
        C::UnknownFormat => Error::Unsupported {
            detail: "this RAR version.".into(),
        },
        C::ECreate | C::EWrite => Error::CannotWrite {
            path: "the destination".into(),
            detail: format!("{e:?}"),
        },
        C::NoMemory => Error::Io {
            detail: "Not enough memory.".into(),
        },
        _ => Error::Io {
            detail: format!("{e:?}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_to_unix() {
        assert_eq!(unix_seconds(1970, 1, 1, 0, 0, 0), 0);
        assert_eq!(unix_seconds(2000, 3, 1, 12, 0, 0), 951_912_000);
    }

    #[test]
    fn single_streams_take_the_name_without_suffix() {
        assert_eq!(
            single_stream_name(Path::new("/a/notes.txt.gz")),
            "notes.txt"
        );
        assert_eq!(single_stream_name(Path::new("data.XZ")), "data");
    }
}
