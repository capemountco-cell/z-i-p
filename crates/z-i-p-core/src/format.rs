//! Recognising what an archive is, from its first bytes rather than its
//! name: a `.zip` that is really a RAR (common with re-uploaded files) opens
//! as a RAR. The name is only a tie-breaker.

use std::fs::File;
use std::io::{self, Read, Seek};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{Error, Result};
use crate::multi::{split_pieces, MultiFile};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    Zip,
    Rar,
    SevenZ,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    TarZst,
    Gz,
    Bz2,
    Xz,
    Zst,
}

impl Format {
    /// The short label shown next to an archive.
    pub fn label(self) -> &'static str {
        match self {
            Format::Zip => "ZIP",
            Format::Rar => "RAR",
            Format::SevenZ => "7Z",
            Format::Tar => "TAR",
            Format::TarGz => "TAR.GZ",
            Format::TarBz2 => "TAR.BZ2",
            Format::TarXz => "TAR.XZ",
            Format::TarZst => "TAR.ZST",
            Format::Gz => "GZ",
            Format::Bz2 => "BZ2",
            Format::Xz => "XZ",
            Format::Zst => "ZST",
        }
    }

    /// A compressed single file rather than a collection of files.
    pub fn is_single_stream(self) -> bool {
        matches!(self, Format::Gz | Format::Bz2 | Format::Xz | Format::Zst)
    }

    /// Formats whose index sits at the front or back, so listing them is
    /// quick. The others must be read through to be listed.
    pub fn has_index(self) -> bool {
        matches!(self, Format::Zip | Format::Rar | Format::SevenZ)
    }
}

/// Where the archive's bytes come from: one file, or the numbered pieces of
/// a split archive.
#[derive(Debug, Clone)]
pub enum Source {
    File(PathBuf),
    Pieces(Vec<PathBuf>),
}

pub trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

impl Source {
    pub fn for_path(path: &Path) -> Result<Source> {
        if !path.exists() {
            return Err(Error::NotFound {
                path: path.display().to_string(),
            });
        }
        if path.is_dir() {
            return Err(Error::Unsupported {
                detail: "that is a folder, not an archive.".into(),
            });
        }
        match split_pieces(path) {
            Some(pieces) if pieces.len() > 1 => Ok(Source::Pieces(pieces)),
            Some(pieces) if pieces.is_empty() => Err(Error::MissingPart {
                detail: "The first piece (.001) is not in the same folder.".into(),
            }),
            _ => Ok(Source::File(path.to_path_buf())),
        }
    }

    /// The file whose name and folder stand for the archive.
    pub fn first(&self) -> &Path {
        match self {
            Source::File(p) => p,
            Source::Pieces(ps) => &ps[0],
        }
    }

    pub fn open(&self) -> Result<Box<dyn ReadSeek>> {
        Ok(match self {
            Source::File(p) => Box::new(File::open(p)?),
            Source::Pieces(ps) => Box::new(MultiFile::open(ps)?),
        })
    }

    pub fn len(&self) -> u64 {
        match self {
            Source::File(p) => p.metadata().map(|m| m.len()).unwrap_or(0),
            Source::Pieces(ps) => ps
                .iter()
                .filter_map(|p| p.metadata().ok())
                .map(|m| m.len())
                .sum(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn piece_count(&self) -> usize {
        match self {
            Source::File(_) => 1,
            Source::Pieces(ps) => ps.len(),
        }
    }
}

/// What the archive is.
pub fn detect(source: &Source) -> Result<Format> {
    let mut head = Vec::with_capacity(512);
    source.open()?.take(512).read_to_end(&mut head)?;
    let name = source
        .first()
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    detect_bytes(&head, &name, || source.open()).ok_or_else(|| Error::Unsupported {
        detail: if head.is_empty() {
            "the file is empty.".into()
        } else {
            "Z-I-P does not recognise this file as an archive.".into()
        },
    })
}

fn is_tar_header(block: &[u8]) -> bool {
    block.len() >= 262 && &block[257..262] == b"ustar"
}

/// The format from the first bytes, the lower-cased file name, and a way to
/// re-open the file to look inside a compressed stream.
pub(crate) fn detect_bytes(
    head: &[u8],
    name: &str,
    reopen: impl Fn() -> Result<Box<dyn ReadSeek>>,
) -> Option<Format> {
    let starts = |magic: &[u8]| head.starts_with(magic);
    if starts(b"PK\x03\x04") || starts(b"PK\x05\x06") || starts(b"PK\x07\x08") {
        return Some(Format::Zip);
    }
    if starts(b"Rar!\x1a\x07") {
        return Some(Format::Rar);
    }
    if starts(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
        return Some(Format::SevenZ);
    }
    if is_tar_header(head) {
        return Some(Format::Tar);
    }

    // A compressed stream: a tarball if what it decompresses to starts with
    // a tar header, or if its name says so (old tars have no magic).
    let stream = if starts(&[0x1F, 0x8B]) {
        Some((Format::Gz, Format::TarGz))
    } else if starts(b"BZh") {
        Some((Format::Bz2, Format::TarBz2))
    } else if starts(&[0xFD, b'7', b'z', b'X', b'Z', 0x00]) {
        Some((Format::Xz, Format::TarXz))
    } else if starts(&[0x28, 0xB5, 0x2F, 0xFD]) {
        Some((Format::Zst, Format::TarZst))
    } else {
        None
    };
    if let Some((single, tarball)) = stream {
        let named_tar = name.contains(".tar.")
            || [".tgz", ".tbz", ".tbz2", ".txz", ".tzst"]
                .iter()
                .any(|s| name.ends_with(s));
        if named_tar {
            return Some(tarball);
        }
        let peek = reopen().ok().and_then(|r| {
            let mut block = Vec::with_capacity(512);
            decompressor(single, r)
                .ok()?
                .take(512)
                .read_to_end(&mut block)
                .ok()?;
            Some(block)
        });
        return Some(if peek.is_some_and(|b| is_tar_header(&b)) {
            tarball
        } else {
            single
        });
    }

    // Some archives are recognised only by name: an old tar without magic.
    if name.ends_with(".tar") && head.len() >= 512 {
        return Some(Format::Tar);
    }
    None
}

/// A reader that undoes the outer compression of a stream format.
pub(crate) fn decompressor<'a>(
    format: Format,
    r: impl Read + 'a,
) -> io::Result<Box<dyn Read + 'a>> {
    Ok(match format {
        Format::Gz | Format::TarGz => Box::new(flate2::read::MultiGzDecoder::new(r)),
        Format::Bz2 | Format::TarBz2 => Box::new(bzip2::read::MultiBzDecoder::new(r)),
        Format::Xz | Format::TarXz => Box::new(liblzma::read::XzDecoder::new_multi_decoder(r)),
        Format::Zst | Format::TarZst => Box::new(zstd::stream::read::Decoder::new(r)?),
        Format::Tar => Box::new(r),
        Format::Zip | Format::Rar | Format::SevenZ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a stream format",
            ))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    fn none() -> Result<Box<dyn ReadSeek>> {
        Err(Error::Io {
            detail: "no reopen".into(),
        })
    }

    #[test]
    fn magic_beats_the_name() {
        assert_eq!(
            detect_bytes(b"Rar!\x1a\x07\x01\x00", "misnamed.zip", none),
            Some(Format::Rar)
        );
        assert_eq!(
            detect_bytes(b"PK\x03\x04rest", "x.rar", none),
            Some(Format::Zip)
        );
        assert_eq!(
            detect_bytes(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C, 0, 4], "a", none),
            Some(Format::SevenZ)
        );
        assert_eq!(detect_bytes(b"hello", "notes.txt", none), None);
    }

    #[test]
    fn tells_a_tarball_from_a_compressed_file() {
        let mut tar_bytes = Vec::new();
        {
            let mut b = tar::Builder::new(&mut tar_bytes);
            let mut h = tar::Header::new_gnu();
            h.set_size(2);
            h.set_cksum();
            b.append_data(&mut h, "a.txt", &b"hi"[..]).unwrap();
            b.finish().unwrap();
        }
        let gz = |data: &[u8]| {
            let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
            e.write_all(data).unwrap();
            e.finish().unwrap()
        };
        let tgz = gz(&tar_bytes);
        let reopen = || -> Result<Box<dyn ReadSeek>> { Ok(Box::new(Cursor::new(tgz.clone()))) };
        assert_eq!(
            detect_bytes(&tgz[..10], "download", reopen),
            Some(Format::TarGz)
        );

        let plain = gz(b"just text");
        let reopen = || -> Result<Box<dyn ReadSeek>> { Ok(Box::new(Cursor::new(plain.clone()))) };
        assert_eq!(
            detect_bytes(&plain[..10], "notes.txt.gz", reopen),
            Some(Format::Gz)
        );
        assert_eq!(
            detect_bytes(&plain[..10], "x.tgz", none),
            Some(Format::TarGz)
        );
    }
}
