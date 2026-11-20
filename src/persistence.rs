//! Saving and loading the app state.
//!
//! State is stored as TOML (human-readable, the usual format for Rust apps)
//! in the platform's data directory:
//!
//! | OS      | path |
//! |---------|------|
//! | Linux   | `~/.local/share/cronostack/state.toml` |
//! | macOS   | `~/Library/Application Support/cronostack/state.toml` |
//! | Windows | `%APPDATA%\cronostack\data\state.toml` |
//!
//! Set the `CRONOSTACK_DATA_DIR` environment variable to use another
//! directory (handy for tests or a portable install).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::theme::ThemeMode;

const FILE_NAME: &str = "state.toml";
const DATA_DIR_ENV: &str = "CRONOSTACK_DATA_DIR";

/// Everything that is saved between runs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppState {
    pub theme: ThemeMode,
    /// Starting a stopwatch pauses all the others.
    pub one_at_a_time: bool,
    pub columns: Vec<ColumnState>,
}

/// One column of the window.
///
/// `#[serde(default)]` lets files written by older versions load even when
/// new fields are added.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColumnState {
    /// Time on the stopwatch, in whole seconds.
    pub elapsed_secs: u64,
    pub project: String,
    pub developer: String,
    pub sections: Sections,
}

/// Which sections of a column are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sections {
    pub settings: bool,
    pub project: bool,
    pub developer: bool,
    pub clock: bool,
    pub alarm: bool,
    pub stopwatch: bool,
    pub countdown: bool,
}

impl Default for Sections {
    /// A new column only shows its stopwatch.
    fn default() -> Self {
        Self {
            settings: false,
            project: false,
            developer: false,
            clock: false,
            alarm: false,
            stopwatch: true,
            countdown: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    #[error("could not access {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("could not read {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("could not serialize state: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// Where the state file lives, and how to read and write it.
#[derive(Debug, Clone)]
pub struct Store {
    path: PathBuf,
}

impl Store {
    /// The default location for this platform (see the module docs).
    pub fn locate() -> Self {
        let dir = std::env::var_os(DATA_DIR_ENV)
            .map(PathBuf::from)
            .or_else(|| {
                directories::ProjectDirs::from("", "", "cronostack")
                    .map(|d| d.data_dir().to_owned())
            })
            .unwrap_or_else(|| PathBuf::from("."));
        Self::in_dir(dir)
    }

    pub fn in_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            path: dir.into().join(FILE_NAME),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads the saved state. A missing file is not an error: it's the
    /// first run, so the default (empty) state is returned.
    pub fn load(&self) -> Result<AppState, PersistError> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(AppState::default()),
            Err(source) => {
                return Err(PersistError::Io {
                    path: self.path.clone(),
                    source,
                });
            }
        };
        toml::from_str(&text).map_err(|source| PersistError::Parse {
            path: self.path.clone(),
            source,
        })
    }

    /// Like [`Store::load`], but never fails: on error it logs, moves the
    /// unreadable file aside (so it isn't overwritten) and starts empty.
    pub fn load_or_default(&self) -> AppState {
        self.load().unwrap_or_else(|e| {
            log::error!("{e}");
            if matches!(e, PersistError::Parse { .. }) {
                let backup = self.path.with_extension("toml.broken");
                match fs::rename(&self.path, &backup) {
                    Ok(()) => log::warn!("unreadable state file moved to {}", backup.display()),
                    Err(e) => log::error!("could not move unreadable state file: {e}"),
                }
            }
            AppState::default()
        })
    }

    /// Writes the state atomically: to a temporary file first, then renamed
    /// over the real one, so a crash mid-write can't corrupt it.
    pub fn save(&self, state: &AppState) -> Result<(), PersistError> {
        let io_err = |source| PersistError::Io {
            path: self.path.clone(),
            source,
        };
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(io_err)?;
        }
        let text = toml::to_string_pretty(state)?;
        let tmp = self.path.with_extension("toml.tmp");
        fs::write(&tmp, text).map_err(io_err)?;
        fs::rename(&tmp, &self.path).map_err(io_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AppState {
        AppState {
            theme: ThemeMode::System,
            one_at_a_time: true,
            columns: vec![ColumnState {
                elapsed_secs: 27,
                project: "website".into(),
                developer: "ana".into(),
                sections: Sections {
                    project: true,
                    ..Sections::default()
                },
            }],
        }
    }

    #[test]
    fn missing_file_is_first_run() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            Store::in_dir(dir.path()).load().unwrap(),
            AppState::default()
        );
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::in_dir(dir.path().join("nested"));
        store.save(&sample()).unwrap();
        assert_eq!(store.load().unwrap(), sample());
    }

    #[test]
    fn broken_file_is_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::in_dir(dir.path());
        fs::write(store.path(), "this is = = not toml").unwrap();
        assert_eq!(store.load_or_default(), AppState::default());
        assert!(dir.path().join("state.toml.broken").exists());
    }

    #[test]
    fn missing_fields_use_defaults() {
        let state: AppState = toml::from_str("[[columns]]\nproject = \"x\"\n").unwrap();
        assert_eq!(state.columns[0].project, "x");
        assert!(state.columns[0].sections.stopwatch);
    }

    #[test]
    fn file_is_readable() {
        let text = toml::to_string_pretty(&sample()).unwrap();
        assert!(text.contains("[[columns]]"), "{text}");
        assert!(text.contains("project = \"website\""), "{text}");
    }
}
