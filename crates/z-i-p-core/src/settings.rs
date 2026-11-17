//! What a person can change, and where things go by default.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::platform;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Destination {
    /// In the folder that holds the archive.
    NextToArchive,
    /// Ask with a folder picker each time.
    Ask,
    /// Always in `folder`.
    Folder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OnOpen {
    /// Unpack straight away.
    Extract,
    /// Show what is inside first, to pick files.
    Browse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub on_open: OnOpen,
    pub destination: Destination,
    pub folder: Option<String>,
    pub smart_folder: bool,
    pub skip_junk: bool,
    pub reveal_when_done: bool,
    pub keep_quarantine: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            on_open: OnOpen::Extract,
            destination: Destination::NextToArchive,
            folder: None,
            smart_folder: true,
            skip_junk: true,
            reveal_when_done: true,
            keep_quarantine: true,
        }
    }
}

/// Settings from `path`. A missing or unreadable file gives the defaults,
/// and unknown or missing fields take their default, so an older or newer
/// settings file never stops the app from opening.
pub fn load(path: &Path) -> Settings {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(
        &tmp,
        serde_json::to_vec_pretty(settings).map_err(std::io::Error::other)?,
    )?;
    fs::rename(tmp, path)
}

/// The folder an archive should unpack into, or `None` when the person has
/// to choose (they asked to be asked, or the usual place cannot be written,
/// such as a read-only disk image).
pub fn destination_for(settings: &Settings, archive: &Path) -> Option<PathBuf> {
    let dir = match settings.destination {
        Destination::Ask => return None,
        Destination::NextToArchive => archive.parent()?.to_path_buf(),
        Destination::Folder => PathBuf::from(settings.folder.as_deref()?),
    };
    platform::is_writable(&dir).then_some(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_survives_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/settings.json");
        assert_eq!(load(&path), Settings::default());
        let s = Settings {
            on_open: OnOpen::Browse,
            skip_junk: false,
            ..Settings::default()
        };
        save(&path, &s).unwrap();
        assert_eq!(load(&path), s);
        fs::write(&path, "{ not json").unwrap();
        assert_eq!(load(&path), Settings::default());
        fs::write(&path, r#"{"skipJunk": false, "fromTheFuture": 1}"#).unwrap();
        assert!(!load(&path).skip_junk);
    }

    #[test]
    fn picks_a_destination() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("a.zip");
        let s = Settings::default();
        assert_eq!(
            destination_for(&s, &archive),
            Some(dir.path().to_path_buf())
        );
        let ask = Settings {
            destination: Destination::Ask,
            ..s.clone()
        };
        assert_eq!(destination_for(&ask, &archive), None);
        let missing = Settings {
            destination: Destination::Folder,
            folder: Some("/nope/never".into()),
            ..s
        };
        assert_eq!(destination_for(&missing, &archive), None);
    }
}
