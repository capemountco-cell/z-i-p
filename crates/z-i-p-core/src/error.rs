//! What can go wrong, in words a person can act on.
//!
//! Every failure from the archive libraries is mapped onto one of these, so
//! the interface can tell a missing password from a damaged download from a
//! full disk, and say what to do about each.

use std::fmt;
use std::io;

use serde::Serialize;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Error {
    /// The file is not there (moved, deleted, or on an unmounted disk).
    NotFound { path: String },
    /// Not an archive Z-I-P can open.
    Unsupported { detail: String },
    /// The archive is encrypted and no password was given.
    PasswordRequired,
    /// A password was given and it does not open the archive.
    WrongPassword,
    /// The destination does not have room for what the archive holds.
    NotEnoughSpace { needed: u64, available: u64 },
    /// One piece of a multi-part archive is missing.
    MissingPart { detail: String },
    /// The archive is damaged or incomplete (often an unfinished download).
    Damaged { detail: String },
    /// The destination cannot be written to.
    CannotWrite { path: String, detail: String },
    /// Anything else the operating system reported.
    Io { detail: String },
    /// The person stopped it.
    Cancelled,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound { path } => write!(f, "{path} could not be found."),
            Error::Unsupported { detail } => {
                write!(f, "This kind of archive is not supported: {detail}")
            }
            Error::PasswordRequired => write!(f, "This archive is protected with a password."),
            Error::WrongPassword => write!(f, "That password does not open this archive."),
            Error::NotEnoughSpace { needed, available } => write!(
                f,
                "Not enough free space: needs {}, {} available.",
                crate::names::human_size(*needed),
                crate::names::human_size(*available)
            ),
            Error::MissingPart { detail } => {
                write!(f, "A part of this multi-part archive is missing. {detail}")
            }
            Error::Damaged { detail } => {
                write!(f, "The archive is damaged or incomplete. {detail}")
            }
            Error::CannotWrite { path, detail } => write!(f, "Cannot write to {path}: {detail}"),
            Error::Io { detail } => write!(f, "{detail}"),
            Error::Cancelled => write!(f, "Stopped."),
        }
    }
}

impl std::error::Error for Error {}

/// The marker an `io::Error` carries when a progress callback asked to stop.
/// Library code only sees an io error, so cancelling travels through it.
pub(crate) const CANCEL_MARKER: &str = "z-i-p: cancelled";

#[cfg(test)]
pub(crate) fn cancelled_io() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, CANCEL_MARKER)
}

pub(crate) fn is_cancel(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::Interrupted && e.to_string().contains(CANCEL_MARKER)
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        if is_cancel(&e) {
            return Error::Cancelled;
        }
        match e.kind() {
            io::ErrorKind::NotFound => Error::NotFound {
                path: e.to_string(),
            },
            io::ErrorKind::UnexpectedEof => Error::Damaged {
                detail: "It ends early, which usually means the download did not finish.".into(),
            },
            io::ErrorKind::StorageFull => Error::NotEnoughSpace {
                needed: 0,
                available: 0,
            },
            io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
                Error::CannotWrite {
                    path: "the destination".into(),
                    detail: e.to_string(),
                }
            }
            _ => {
                // ENOSPC on systems where the kind is not mapped yet.
                if e.raw_os_error() == Some(libc::ENOSPC) {
                    Error::NotEnoughSpace {
                        needed: 0,
                        available: 0,
                    }
                } else {
                    Error::Io {
                        detail: e.to_string(),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_survives_a_trip_through_io_error() {
        let e: Error = cancelled_io().into();
        assert_eq!(e, Error::Cancelled);
    }

    #[test]
    fn early_end_reads_as_damaged() {
        let e: Error = io::Error::new(io::ErrorKind::UnexpectedEof, "eof").into();
        assert!(matches!(e, Error::Damaged { .. }));
    }
}
