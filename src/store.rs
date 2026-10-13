//! Persistence port for the engine's learned state, with a file-backed default.
//!
//! The file layout is the one hosts already have on disk, read unchanged:
//!
//! - `slang-index.json`: the slang index.
//! - `meme-index.json`: learned reaction GIFs.
//! - `remixed.json`: ids of delivered remixed messages, newest last, capped at
//!   [`REMIXED_CAP`].
//!
//! Files are written atomically (unique temp file, then rename).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{IndexPolicy, MemeIndex, MemeIndexPolicy, SlangIndex};

/// Slang index file name.
pub const INDEX_FILE: &str = "slang-index.json";
/// Meme index file name.
pub const MEME_INDEX_FILE: &str = "meme-index.json";
/// Remixed-ids file name.
pub const REMIXED_FILE: &str = "remixed.json";
/// Remixed-message ids remembered (oldest dropped first).
pub const REMIXED_CAP: usize = 4000;

/// Where the engine's learned state lives. Loads never fail: unreadable state
/// is a fresh start. Saves report the I/O error so the host can log it.
pub trait IndexStore: Send + Sync {
    /// The slang index, or an empty one.
    fn load_slang(&self) -> SlangIndex;
    /// The meme index, or an empty one.
    fn load_memes(&self) -> MemeIndex;
    /// Remixed ids, oldest first, at most [`REMIXED_CAP`].
    fn load_remixed(&self) -> VecDeque<String>;
    /// Persist the slang index.
    fn save_slang(&self, index: &SlangIndex) -> std::io::Result<()>;
    /// Persist the meme index.
    fn save_memes(&self, index: &MemeIndex) -> std::io::Result<()>;
    /// Persist the remixed ids (capped to [`REMIXED_CAP`] by the store).
    fn save_remixed(&self, ids: &VecDeque<String>) -> std::io::Result<()>;
}

/// Drop the oldest ids beyond [`REMIXED_CAP`].
pub fn cap_remixed(ids: &mut VecDeque<String>) {
    while ids.len() > REMIXED_CAP {
        ids.pop_front();
    }
}

/// The default [`IndexStore`]: JSON files in one directory.
#[derive(Clone, Debug)]
pub struct FileIndexStore {
    dir: PathBuf,
}

impl FileIndexStore {
    /// A store over `dir`. The directory is created on first save.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The directory holding the state files.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn write(&self, file: &str, contents: &str) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        write_atomic(&self.dir.join(file), contents)
    }
}

impl IndexStore for FileIndexStore {
    fn load_slang(&self) -> SlangIndex {
        match std::fs::read_to_string(self.dir.join(INDEX_FILE)) {
            Ok(json) => SlangIndex::from_json(&json, IndexPolicy::default()).unwrap_or_else(|e| {
                tracing::warn!("[tinymemes] slang index unreadable, starting fresh: {e}");
                SlangIndex::new(IndexPolicy::default())
            }),
            Err(_) => SlangIndex::new(IndexPolicy::default()),
        }
    }

    fn load_memes(&self) -> MemeIndex {
        let policy = MemeIndexPolicy::default();
        match std::fs::read_to_string(self.dir.join(MEME_INDEX_FILE)) {
            Ok(json) => MemeIndex::from_json(&json, policy).unwrap_or_else(|e| {
                tracing::warn!("[tinymemes] meme index unreadable, starting fresh: {e}");
                MemeIndex::new(policy)
            }),
            Err(_) => MemeIndex::new(policy),
        }
    }

    fn load_remixed(&self) -> VecDeque<String> {
        let mut ids: VecDeque<String> = std::fs::read_to_string(self.dir.join(REMIXED_FILE))
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        cap_remixed(&mut ids);
        ids
    }

    fn save_slang(&self, index: &SlangIndex) -> std::io::Result<()> {
        self.write(INDEX_FILE, &index.to_json())
    }

    fn save_memes(&self, index: &MemeIndex) -> std::io::Result<()> {
        self.write(MEME_INDEX_FILE, &index.to_json())
    }

    fn save_remixed(&self, ids: &VecDeque<String>) -> std::io::Result<()> {
        let mut capped = ids.clone();
        cap_remixed(&mut capped);
        let json = serde_json::to_string(&capped).unwrap_or_default();
        self.write(REMIXED_FILE, &json)
    }
}

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Write `contents` to `path` atomically: a unique temp file in the same
/// directory, then a rename over the target. Concurrent writers each rename a
/// complete file; a crash never leaves a half-written one.
pub fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("state");
    let tmp = path.with_file_name(format!(".{name}.{}.{seq}.tmp", std::process::id()));
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
