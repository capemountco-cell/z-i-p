//! Split archives: `Photos.7z.001`, `.002`… are one archive cut into
//! pieces. `MultiFile` reads the pieces back to back as if they were one
//! file, so the 7z and zip readers never know.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Every piece of a split archive, in order, starting from whichever piece
/// was opened. `None` when the path is not a numbered piece.
pub fn split_pieces(path: &Path) -> Option<Vec<PathBuf>> {
    let name = path.file_name()?.to_str()?;
    let (head, tail) = name.rsplit_once('.')?;
    if tail.len() != 3 || !tail.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let dir = path.parent().unwrap_or(Path::new(""));
    let pieces: Vec<PathBuf> = (1..=999)
        .map(|n| dir.join(format!("{head}.{n:03}")))
        .take_while(|p| p.is_file())
        .collect();
    Some(pieces)
}

pub struct MultiFile {
    files: Vec<File>,
    /// Offset at which each piece starts, plus the total length at the end.
    starts: Vec<u64>,
    pos: u64,
}

impl MultiFile {
    pub fn open(paths: &[PathBuf]) -> Result<Self> {
        if paths.is_empty() {
            return Err(Error::MissingPart {
                detail: "The first piece (.001) was not found next to this one.".into(),
            });
        }
        let mut files = Vec::with_capacity(paths.len());
        let mut starts = vec![0];
        for p in paths {
            let f = File::open(p)?;
            let len = f.metadata()?.len();
            starts.push(starts.last().copied().unwrap_or(0) + len);
            files.push(f);
        }
        Ok(Self {
            files,
            starts,
            pos: 0,
        })
    }

    pub fn len(&self) -> u64 {
        *self.starts.last().unwrap_or(&0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Read for MultiFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() || self.pos >= self.len() {
            return Ok(0);
        }
        // The piece holding `pos`: the last start at or before it.
        let i = self.starts.partition_point(|&s| s <= self.pos) - 1;
        let offset = self.pos - self.starts[i];
        let room = (self.starts[i + 1] - self.pos).min(buf.len() as u64) as usize;
        let f = &mut self.files[i];
        f.seek(SeekFrom::Start(offset))?;
        let n = f.read(&mut buf[..room])?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for MultiFile {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(n) => n as i128,
            SeekFrom::End(n) => self.len() as i128 + n as i128,
            SeekFrom::Current(n) => self.pos as i128 + n as i128,
        };
        if target < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek before start",
            ));
        }
        self.pos = target as u64;
        Ok(self.pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_pieces_as_one_file() {
        let dir = tempfile::tempdir().unwrap();
        let data: Vec<u8> = (0..=255u8).cycle().take(2500).collect();
        for (i, chunk) in data.chunks(1000).enumerate() {
            std::fs::write(dir.path().join(format!("a.bin.{:03}", i + 1)), chunk).unwrap();
        }
        let pieces = split_pieces(&dir.path().join("a.bin.002")).unwrap();
        assert_eq!(pieces.len(), 3);
        let mut m = MultiFile::open(&pieces).unwrap();
        let mut all = Vec::new();
        m.read_to_end(&mut all).unwrap();
        assert_eq!(all, data);

        m.seek(SeekFrom::Start(995)).unwrap();
        let mut ten = [0u8; 10];
        m.read_exact(&mut ten).unwrap();
        assert_eq!(&ten, &data[995..1005]);
        assert_eq!(m.seek(SeekFrom::End(-1)).unwrap(), 2499);
    }

    #[test]
    fn plain_files_are_not_pieces() {
        assert!(split_pieces(Path::new("a.zip")).is_none());
        assert!(split_pieces(Path::new("a.mp3")).is_none());
    }
}
