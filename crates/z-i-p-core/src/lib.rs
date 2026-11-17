//! Z-I-P's logic: recognising archives, listing them and unpacking them
//! safely.
//!
//! Everything that makes a decision lives here, not in the Tauri crate and not
//! in React. This crate does not depend on Tauri, so it builds and its tests
//! run on Linux in CI, without a window, a webview or a Mac.

pub mod error;
pub mod extract;
pub mod format;
pub mod history;
pub mod jobs;
pub mod list;
pub mod multi;
pub mod names;
pub mod platform;
pub mod settings;

pub use error::{Error, Result};
pub use extract::{extract, Outcome, Progress, Request};
pub use format::Format;
pub use list::{list, Entry, Listing};
