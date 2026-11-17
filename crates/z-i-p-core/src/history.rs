//! A record of what was unpacked and where it went, so "where did my files
//! go?" always has an answer.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

const KEEP: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub archive: String,
    /// Where the result went; empty when it failed.
    pub output: String,
    /// Seconds since 1970.
    pub at: i64,
    pub files: u64,
    pub bytes: u64,
    /// Why it failed, or `None` if it worked.
    pub error: Option<String>,
}

impl Item {
    /// The record of one extraction, finished now.
    pub fn of(req: &crate::Request, result: &crate::Result<crate::Outcome>) -> Item {
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let archive = req.archive.to_string_lossy().into_owned();
        match result {
            Ok(o) => Item {
                archive,
                output: o.output.clone(),
                at,
                files: o.files,
                bytes: o.bytes,
                error: None,
            },
            Err(e) => Item {
                archive,
                output: String::new(),
                at,
                files: 0,
                bytes: 0,
                error: Some(e.to_string()),
            },
        }
    }
}

pub fn load(path: &Path) -> Vec<Item> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Add `item` at the top, keeping the newest `KEEP`.
pub fn record(path: &Path, item: Item) -> std::io::Result<Vec<Item>> {
    let mut items = load(path);
    items.insert(0, item);
    items.truncate(KEEP);
    save(path, &items)?;
    Ok(items)
}

pub fn clear(path: &Path) -> std::io::Result<()> {
    save(path, &[])
}

fn save(path: &Path, items: &[Item]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(
        &tmp,
        serde_json::to_vec(items).map_err(std::io::Error::other)?,
    )?;
    fs::rename(tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(n: i64) -> Item {
        Item {
            archive: format!("a{n}.zip"),
            output: "out".into(),
            at: n,
            files: 1,
            bytes: 2,
            error: None,
        }
    }

    #[test]
    fn newest_first_and_capped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        for n in 0..(KEEP as i64 + 5) {
            record(&path, item(n)).unwrap();
        }
        let items = load(&path);
        assert_eq!(items.len(), KEEP);
        assert_eq!(items[0].at, KEEP as i64 + 4);
        clear(&path).unwrap();
        assert!(load(&path).is_empty());
    }
}
