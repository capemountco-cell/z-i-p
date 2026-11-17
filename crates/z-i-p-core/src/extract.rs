//! Unpacking, safely.
//!
//! The rules, each one a complaint about other unarchivers:
//!
//! - **Nothing half-done is left behind.** Files go into a hidden staging
//!   folder beside the destination and appear under their real name only
//!   once every file is out. If anything fails, or the person stops it, the
//!   staging folder is deleted.
//! - **Nothing is overwritten.** The result gets a fresh name (`Photos 2`)
//!   if `Photos` already exists. The archive itself is never touched.
//! - **Room is checked first.** When the archive records its sizes, a full
//!   disk is reported before a single byte is written.
//! - **No file lands outside the destination,** whatever its name says.
//! - **No nested folders.** An archive holding one folder unpacks to that
//!   folder, not to `Photos/Photos/`.

use std::cell::Cell;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{is_cancel, Error, Result};
use crate::format::{decompressor, detect, Format, Source};
use crate::list::{list, map_7z, map_rar, map_zip, single_stream_name, unix_seconds};
use crate::names::{archive_stem, decode_name, display_path, is_junk, safe_relative, unique_path};
use crate::platform;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub archive: PathBuf,
    /// The folder the result is placed in.
    pub destination: PathBuf,
    #[serde(default)]
    pub password: Option<String>,
    /// Only these entries (paths as listed; a folder includes its contents).
    /// `None` unpacks everything.
    #[serde(default)]
    pub only: Option<Vec<String>>,
    #[serde(default = "yes")]
    pub skip_junk: bool,
    /// Unpack an archive holding a single folder (or file) straight to it.
    #[serde(default = "yes")]
    pub smart_folder: bool,
    /// Mark unpacked files as downloaded when the archive was, so macOS
    /// checks apps inside it as it would have checked the archive.
    #[serde(default = "yes")]
    pub keep_quarantine: bool,
}

fn yes() -> bool {
    true
}

impl Request {
    pub fn new(archive: impl Into<PathBuf>, destination: impl Into<PathBuf>) -> Self {
        Self {
            archive: archive.into(),
            destination: destination.into(),
            password: None,
            only: None,
            skip_junk: true,
            smart_folder: true,
            keep_quarantine: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    /// The folder or file that was created.
    pub output: String,
    pub files: u64,
    pub bytes: u64,
    /// Clutter (`__MACOSX`, `.DS_Store`…) left out.
    pub skipped: u64,
    /// Entries refused because their names would escape the destination.
    pub unsafe_names: u64,
}

/// How an extraction tells the outside world how far it has got, and learns
/// that it should stop.
pub trait Progress {
    /// `done` of `total` bytes. `total` is 0 when it is not known yet.
    fn report(&self, done: u64, total: u64, current: &str);
    fn should_stop(&self) -> bool;
}

/// A `Progress` that ignores reports and never stops.
pub struct Silent;
impl Progress for Silent {
    fn report(&self, _: u64, _: u64, _: &str) {}
    fn should_stop(&self) -> bool {
        false
    }
}

pub fn extract(req: &Request, progress: &dyn Progress) -> Result<Outcome> {
    let source = Source::for_path(&req.archive)?;
    let format = detect(&source)?;

    // What will be written, for the space check and the progress bar.
    let total = if format.has_index() {
        let listing = list(&req.archive, req.password.as_deref())?;
        let sel = Selection {
            only: req.only.as_deref(),
            skip_junk: req.skip_junk,
        };
        listing
            .entries
            .iter()
            .filter(|e| !e.is_dir && sel.wants_str(&e.path))
            .map(|e| e.size.unwrap_or(0))
            .sum()
    } else {
        0
    };

    if !req.destination.exists() {
        fs::create_dir_all(&req.destination).map_err(|e| cannot_write(&req.destination, e))?;
    }
    if let Some(available) = platform::free_space(&req.destination) {
        // A little headroom: the filesystem needs some for its own records.
        let needed = total + total / 50;
        if needed > available {
            return Err(Error::NotEnoughSpace { needed, available });
        }
    }

    let staging = Staging::create(&req.destination)?;
    let mut sink = Sink {
        root: staging.dir.clone(),
        progress,
        selection: Selection {
            only: req.only.as_deref(),
            skip_junk: req.skip_junk,
        },
        total,
        done: 0,
        external: None,
        last_report: 0,
        current: String::new(),
        files: 0,
        bytes: 0,
        skipped: 0,
        unsafe_names: 0,
    };
    let password = req.password.as_deref().filter(|p| !p.is_empty());

    match format {
        Format::Zip => unpack_zip(&source, password, &mut sink)?,
        Format::SevenZ => unpack_7z(&source, password, &mut sink)?,
        Format::Rar => unpack_rar(source.first(), password, &mut sink)?,
        f if f.is_single_stream() => unpack_single(&source, f, &mut sink)?,
        f => unpack_tar(&source, f, &mut sink)?,
    }
    sink.report(true);

    let output = staging.finish(
        &req.destination,
        &archive_stem(source.first()),
        req.smart_folder,
    )?;
    if req.keep_quarantine {
        platform::copy_quarantine(source.first(), &output);
    }
    Ok(Outcome {
        output: output.to_string_lossy().into_owned(),
        files: sink.files,
        bytes: sink.bytes,
        skipped: sink.skipped,
        unsafe_names: sink.unsafe_names,
    })
}

fn cannot_write(path: &Path, e: io::Error) -> Error {
    match Error::from(e) {
        Error::CannotWrite { detail, .. } => Error::CannotWrite {
            path: path.display().to_string(),
            detail,
        },
        other => other,
    }
}

/// Which entries to write.
struct Selection<'a> {
    only: Option<&'a [String]>,
    skip_junk: bool,
}

impl Selection<'_> {
    fn wants_str(&self, path: &str) -> bool {
        if self.skip_junk && is_junk(Path::new(path)) {
            return false;
        }
        match self.only {
            None => true,
            Some(only) => only.iter().any(|s| {
                let s = s.trim_end_matches('/');
                path == s || (path.starts_with(s) && path.as_bytes().get(s.len()) == Some(&b'/'))
            }),
        }
    }
}

/// The hidden folder files are unpacked into before they are given their
/// real place. Dropping it without `finish` deletes it and everything in it.
struct Staging {
    dir: PathBuf,
    keep: bool,
}

impl Staging {
    fn create(dest: &Path) -> Result<Self> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = dest.join(format!(".z-i-p-unpacking-{}-{nanos}", std::process::id()));
        fs::create_dir(&dir).map_err(|e| cannot_write(dest, e))?;
        Ok(Self { dir, keep: false })
    }

    /// Move the result into place and return where it went.
    fn finish(mut self, dest: &Path, stem: &str, smart: bool) -> Result<PathBuf> {
        let items: Vec<PathBuf> = fs::read_dir(&self.dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        if items.is_empty() {
            return Err(Error::Unsupported {
                detail: "there is nothing in it to unpack.".into(),
            });
        }
        if smart && items.len() == 1 {
            let item = &items[0];
            let name = item
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let target = unique_path(dest, &name);
            fs::rename(item, &target)?;
            return Ok(target);
        }
        let target = unique_path(dest, stem);
        fs::rename(&self.dir, &target)?;
        self.keep = true;
        Ok(target)
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
}

/// Where entries are written, and the bookkeeping that goes with it.
struct Sink<'a> {
    root: PathBuf,
    progress: &'a dyn Progress,
    selection: Selection<'a>,
    total: u64,
    done: u64,
    /// For streamed formats: compressed bytes read so far, which measures
    /// progress better than bytes written when the total is unknown.
    external: Option<Rc<Cell<u64>>>,
    last_report: u64,
    current: String,
    files: u64,
    bytes: u64,
    skipped: u64,
    unsafe_names: u64,
}

const REPORT_EVERY: u64 = 512 * 1024;

impl Sink<'_> {
    /// The safe path for an entry name, or `None` (counted) if it is unsafe.
    fn safe(&mut self, name: &str) -> Option<PathBuf> {
        let p = safe_relative(name);
        if p.is_none() && !name.trim_matches(['/', '\\', '.']).is_empty() {
            self.unsafe_names += 1;
        }
        p
    }

    /// Whether to write this entry; clutter that is left out is counted.
    fn wants(&mut self, rel: &Path, is_dir: bool) -> bool {
        let shown = display_path(rel);
        if self.selection.wants_str(&shown) {
            return true;
        }
        if !is_dir && self.selection.skip_junk && is_junk(rel) {
            self.skipped += 1;
        }
        false
    }

    fn check_stop(&self) -> Result<()> {
        if self.progress.should_stop() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }

    fn report(&mut self, force: bool) {
        let done = self.external.as_ref().map(|c| c.get()).unwrap_or(self.done);
        if force || done.saturating_sub(self.last_report) >= REPORT_EVERY {
            self.last_report = done;
            self.progress.report(done, self.total, &self.current);
        }
    }

    fn start(&mut self, rel: &Path) -> Result<PathBuf> {
        self.check_stop()?;
        self.current = display_path(rel);
        self.report(true);
        let target = self.root.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| cannot_write(&self.root, e))?;
        }
        Ok(target)
    }

    fn dir(&mut self, rel: &Path) -> Result<()> {
        let target = self.root.join(rel);
        fs::create_dir_all(&target).map_err(|e| cannot_write(&self.root, e))?;
        Ok(())
    }

    /// Write one file from `reader`. Read failures are the archive's fault
    /// and are explained by `on_read`; write failures are the destination's.
    fn file(
        &mut self,
        rel: &Path,
        reader: &mut dyn Read,
        mode: Option<u32>,
        modified: Option<SystemTime>,
        on_read: &dyn Fn(io::Error) -> Error,
    ) -> Result<()> {
        let target = self.start(rel)?;
        // A later entry with the same name replaces an earlier one, as in
        // every unarchiver; a folder in the way is an error in the archive.
        if target.is_dir() {
            return Ok(());
        }
        let mut out = File::create(&target).map_err(|e| cannot_write(&self.root, e))?;
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let n = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted && !is_cancel(&e) => continue,
                Err(e) if is_cancel(&e) => return Err(Error::Cancelled),
                Err(e) => return Err(on_read(e)),
            };
            out.write_all(&buf[..n])
                .map_err(|e| cannot_write(&self.root, e))?;
            self.done += n as u64;
            self.bytes += n as u64;
            self.report(false);
            if self.progress.should_stop() {
                return Err(Error::Cancelled);
            }
        }
        if let Some(t) = modified {
            let _ = out.set_modified(t);
        }
        drop(out);
        set_mode(&target, mode);
        self.files += 1;
        Ok(())
    }

    /// A symbolic link, only if it points somewhere inside the result.
    fn symlink(&mut self, rel: &Path, link_target: &str) -> Result<()> {
        let parent = rel.parent().map(display_path).unwrap_or_default();
        let resolved = if parent.is_empty() {
            link_target.to_string()
        } else {
            format!("{parent}/{link_target}")
        };
        if link_target.starts_with('/')
            || link_target.contains('\0')
            || safe_relative(&resolved).is_none()
        {
            self.unsafe_names += 1;
            return Ok(());
        }
        let target = self.start(rel)?;
        let _ = fs::remove_file(&target);
        #[cfg(unix)]
        std::os::unix::fs::symlink(link_target, &target)
            .map_err(|e| cannot_write(&self.root, e))?;
        self.files += 1;
        Ok(())
    }
}

/// Unix permissions from the archive, minus anything special (setuid and
/// friends), and always readable and writable by the owner.
fn set_mode(path: &Path, mode: Option<u32>) {
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode((mode & 0o777) | 0o600));
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
}

fn damaged(e: io::Error) -> Error {
    match e.kind() {
        io::ErrorKind::UnexpectedEof => Error::from(e),
        _ => Error::Damaged {
            detail: e.to_string(),
        },
    }
}

fn unpack_zip(source: &Source, password: Option<&str>, sink: &mut Sink) -> Result<()> {
    let mut zip = zip::ZipArchive::new(source.open()?).map_err(map_zip)?;
    for i in 0..zip.len() {
        let (name, is_dir, encrypted, is_symlink, mode, modified) = {
            let f = zip.by_index_raw(i).map_err(map_zip)?;
            let modified = f.last_modified().map(|d| {
                let secs = unix_seconds(
                    d.year() as i64,
                    d.month() as i64,
                    d.day() as i64,
                    d.hour(),
                    d.minute(),
                    d.second(),
                );
                UNIX_EPOCH + Duration::from_secs(secs.max(0) as u64)
            });
            (
                decode_name(f.name_raw()),
                f.is_dir(),
                f.encrypted(),
                f.is_symlink(),
                f.unix_mode(),
                modified,
            )
        };
        let Some(rel) = sink.safe(&name) else {
            continue;
        };
        if !sink.wants(&rel, is_dir) {
            continue;
        }
        if is_dir {
            sink.dir(&rel)?;
            continue;
        }
        let mut f = match (encrypted, password) {
            (true, None) => return Err(Error::PasswordRequired),
            (true, Some(pw)) => zip.by_index_decrypt(i, pw.as_bytes()),
            (false, _) => zip.by_index(i),
        }
        .map_err(map_zip)?;
        if is_symlink {
            let mut target = String::new();
            (&mut f)
                .take(4096)
                .read_to_string(&mut target)
                .map_err(damaged)?;
            drop(f);
            sink.symlink(&rel, &target)?;
            continue;
        }
        // A wrong ZipCrypto password can pass the quick check and fail only
        // when the data does not match its checksum.
        let on_read = |e: io::Error| {
            if encrypted {
                Error::WrongPassword
            } else {
                damaged(e)
            }
        };
        sink.file(&rel, &mut f, mode, modified, &on_read)?;
    }
    Ok(())
}

fn unpack_7z(source: &Source, password: Option<&str>, sink: &mut Sink) -> Result<()> {
    let pw = password
        .map(sevenz_rust2::Password::from)
        .unwrap_or_else(sevenz_rust2::Password::empty);
    let has_pw = password.is_some();
    let mut reader =
        sevenz_rust2::ArchiveReader::new(source.open()?, pw).map_err(|e| map_7z(e, has_pw))?;
    let archive = reader.archive();
    if !has_pw && (0..archive.files.len()).any(|i| archive.is_file_encrypted(i)) {
        return Err(Error::PasswordRequired);
    }

    let mut failure: Option<Error> = None;
    let result = reader.for_each_entries(|entry, r| {
        let step = (|| -> Result<()> {
            if entry.is_anti_item() {
                return Ok(());
            }
            let Some(rel) = sink.safe(entry.name()) else {
                return Ok(());
            };
            if !sink.wants(&rel, entry.is_directory()) {
                return Ok(());
            }
            if entry.is_directory() {
                return sink.dir(&rel);
            }
            let modified = entry
                .has_last_modified_date
                .then(|| SystemTime::from(entry.last_modified_date()));
            // p7zip keeps Unix permissions in the high half of the attributes.
            let attrs = entry.windows_attributes();
            let mode = (entry.has_windows_attributes && attrs & 0x8000 != 0).then_some(attrs >> 16);
            let on_read = |e: io::Error| {
                if has_pw {
                    Error::WrongPassword
                } else {
                    damaged(e)
                }
            };
            sink.file(&rel, r, mode, modified, &on_read)
        })();
        match step {
            Ok(()) => Ok(true),
            Err(e) => {
                failure = Some(e);
                Err(sevenz_rust2::Error::Other("stopped by z-i-p".into()))
            }
        }
    });
    if let Some(e) = failure {
        return Err(e);
    }
    result.map_err(|e| match e {
        sevenz_rust2::Error::Io(io, _) if is_cancel(&io) => Error::Cancelled,
        e => map_7z(e, has_pw),
    })
}

fn unpack_rar(path: &Path, password: Option<&str>, sink: &mut Sink) -> Result<()> {
    let archive = match password {
        Some(pw) => unrar::Archive::with_password(path, pw.as_bytes()),
        None => unrar::Archive::new(path),
    };
    let archive = if archive.is_multipart() {
        archive.as_first_part()
    } else {
        archive
    };
    let mut cursor = archive.open_for_processing().map_err(map_rar)?;
    while let Some(header) = cursor.read_header().map_err(map_rar)? {
        sink.check_stop()?;
        let entry = header.entry();
        let (name, is_dir, size, encrypted) = (
            entry.filename.to_string_lossy().into_owned(),
            entry.is_directory(),
            entry.unpacked_size,
            entry.is_encrypted(),
        );
        let rel = match sink.safe(&name) {
            Some(rel) if sink.wants(&rel, is_dir) => rel,
            _ => {
                cursor = header.skip().map_err(map_rar)?;
                continue;
            }
        };
        if is_dir {
            sink.dir(&rel)?;
            cursor = header.skip().map_err(map_rar)?;
            continue;
        }
        if encrypted && password.is_none() {
            return Err(Error::PasswordRequired);
        }
        let target = sink.start(&rel)?;
        cursor = header.extract_to(&target).map_err(map_rar)?;
        sink.done += size;
        sink.bytes += size;
        sink.files += 1;
        sink.report(false);
    }
    Ok(())
}

/// Counts the bytes read through it, for progress on streamed formats.
struct Counting<R> {
    inner: R,
    count: Rc<Cell<u64>>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.count.set(self.count.get() + n as u64);
        Ok(n)
    }
}

fn counted(source: &Source, sink: &mut Sink) -> Result<Counting<Box<dyn crate::format::ReadSeek>>> {
    let count = Rc::new(Cell::new(0));
    sink.external = Some(count.clone());
    sink.total = source.len();
    Ok(Counting {
        inner: source.open()?,
        count,
    })
}

fn unpack_single(source: &Source, format: Format, sink: &mut Sink) -> Result<()> {
    let name = single_stream_name(source.first());
    let rel = PathBuf::from(&name);
    let input = counted(source, sink)?;
    let mut reader = decompressor(format, input)?;
    let modified = source
        .first()
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok());
    sink.file(&rel, &mut reader, None, modified, &damaged)
}

fn unpack_tar(source: &Source, format: Format, sink: &mut Sink) -> Result<()> {
    let input = counted(source, sink)?;
    let mut tar = tar::Archive::new(decompressor(format, input)?);
    for entry in tar.entries().map_err(damaged)? {
        let mut entry = entry.map_err(damaged)?;
        let kind = entry.header().entry_type();
        let name = decode_name(&entry.path_bytes());
        let Some(rel) = sink.safe(&name) else {
            continue;
        };
        if !sink.wants(&rel, kind.is_dir()) {
            continue;
        }
        if kind.is_dir() {
            sink.dir(&rel)?;
        } else if kind.is_symlink() {
            let target = entry
                .link_name_bytes()
                .map(|b| decode_name(&b))
                .unwrap_or_default();
            sink.symlink(&rel, &target)?;
        } else if kind.is_hard_link() {
            // A second name for a file written earlier: copy it.
            let linked = entry
                .link_name_bytes()
                .map(|b| decode_name(&b))
                .and_then(|n| safe_relative(&n));
            if let Some(from) = linked.map(|l| sink.root.join(l)).filter(|p| p.is_file()) {
                let to = sink.start(&rel)?;
                fs::copy(&from, &to).map_err(|e| cannot_write(&sink.root, e))?;
                sink.files += 1;
            }
        } else if kind.is_file()
            || kind == tar::EntryType::Continuous
            || kind == tar::EntryType::GNUSparse
        {
            let mode = entry.header().mode().ok();
            let modified = entry
                .header()
                .mtime()
                .ok()
                .map(|t| UNIX_EPOCH + Duration::from_secs(t));
            sink.file(&rel, &mut entry, mode, modified, &damaged)?;
        }
    }
    Ok(())
}
