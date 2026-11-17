//! The few things that ask the operating system directly: free space,
//! Finder, and the "downloaded from the internet" mark.

use std::path::Path;
use std::process::Command;

/// Bytes free for an ordinary user on the volume holding `path`.
pub fn free_space(path: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let c = CString::new(path.as_os_str().as_bytes()).ok()?;
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        // SAFETY: `c` is a valid NUL-terminated path and `st` is writable.
        if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
            return None;
        }
        #[allow(clippy::unnecessary_cast)]
        Some(st.f_bavail as u64 * st.f_frsize as u64)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

/// Select the item in a Finder window.
pub fn reveal(path: &Path) -> std::io::Result<()> {
    Command::new("open").arg("-R").arg(path).spawn().map(|_| ())
}

/// Open with the default app (a folder opens in Finder).
pub fn open(path: &Path) -> std::io::Result<()> {
    Command::new("open").arg(path).spawn().map(|_| ())
}

/// Can files be created in `dir`? Checked by trying, since permissions,
/// read-only disk images and sandboxing all say no in different ways.
pub fn is_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".z-i-p-write-test-{}", std::process::id()));
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Copy macOS's quarantine mark from the archive to everything unpacked
/// from it, as Archive Utility does, so Gatekeeper still checks an app that
/// arrived inside a download. Does nothing if the archive has no mark.
pub fn copy_quarantine(archive: &Path, output: &Path) {
    #[cfg(target_os = "macos")]
    {
        let Some(mark) = xattr::get(archive, "com.apple.quarantine") else {
            return;
        };
        let mut stack = vec![output.to_path_buf()];
        while let Some(p) = stack.pop() {
            xattr::set(&p, "com.apple.quarantine", &mark);
            if p.symlink_metadata().map(|m| m.is_dir()).unwrap_or(false) {
                if let Ok(rd) = std::fs::read_dir(&p) {
                    stack.extend(rd.filter_map(|e| e.ok()).map(|e| e.path()));
                }
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (archive, output);
}

#[cfg(target_os = "macos")]
mod xattr {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    fn c(path: &Path) -> Option<CString> {
        CString::new(path.as_os_str().as_bytes()).ok()
    }

    pub fn get(path: &Path, name: &str) -> Option<Vec<u8>> {
        let (p, n) = (c(path)?, CString::new(name).ok()?);
        let mut buf = vec![0u8; 1024];
        // SAFETY: valid C strings and a buffer of the stated length.
        let len = unsafe {
            libc::getxattr(
                p.as_ptr(),
                n.as_ptr(),
                buf.as_mut_ptr().cast(),
                buf.len(),
                0,
                libc::XATTR_NOFOLLOW,
            )
        };
        if len <= 0 {
            return None;
        }
        buf.truncate(len as usize);
        Some(buf)
    }

    pub fn set(path: &Path, name: &str, value: &[u8]) {
        let (Some(p), Ok(n)) = (c(path), CString::new(name)) else {
            return;
        };
        // SAFETY: valid C strings and a value of the stated length.
        unsafe {
            libc::setxattr(
                p.as_ptr(),
                n.as_ptr(),
                value.as_ptr().cast(),
                value.len(),
                0,
                libc::XATTR_NOFOLLOW,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_free_space_and_writability() {
        let dir = tempfile::tempdir().unwrap();
        assert!(free_space(dir.path()).unwrap_or(1) > 0);
        assert!(is_writable(dir.path()));
        assert!(!is_writable(&dir.path().join("missing")));
    }
}
