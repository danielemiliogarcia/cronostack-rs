//! Saving and loading the app state.
//!
//! State is stored as TOML (human-readable, the usual format for Rust apps).
//! Where it's stored depends on the platform:
//!
//! | Platform | location |
//! |----------|----------|
//! | Linux    | `~/.local/share/cronostack/state.toml` |
//! | macOS    | `~/Library/Application Support/cronostack/state.toml` |
//! | Windows  | `%APPDATA%\cronostack\data\state.toml` |
//! | Web      | the browser's `localStorage`, key `cronostack.state` |
//!
//! On the desktop, set the `CRONOSTACK_DATA_DIR` environment variable to use
//! another directory (handy for tests or a portable install).

use serde::{Deserialize, Serialize};

use crate::theme::ThemeMode;

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
    #[error("could not access {location}: {message}")]
    Storage { location: String, message: String },
    #[error("could not read {location}: {source}")]
    Parse {
        location: String,
        source: toml::de::Error,
    },
    #[error("could not serialize state: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// Where the state lives, and how to read and write it. The TOML handling
/// is shared; only the raw read/write differs per platform (see `backend`).
#[derive(Debug, Clone)]
pub struct Store {
    backend: backend::Backend,
}

impl Store {
    /// The default location for this platform (see the module docs).
    pub fn locate() -> Self {
        Self {
            backend: backend::Backend::locate(),
        }
    }

    /// Where the state is kept, for showing to the user.
    pub fn location(&self) -> String {
        self.backend.location()
    }

    /// Loads the saved state. Nothing saved yet is not an error: it's the
    /// first run, so the default (empty) state is returned.
    pub fn load(&self) -> Result<AppState, PersistError> {
        match self.backend.read()? {
            None => Ok(AppState::default()),
            Some(text) => toml::from_str(&text).map_err(|source| PersistError::Parse {
                location: self.location(),
                source,
            }),
        }
    }

    /// Like [`Store::load`], but never fails: on error it logs, moves the
    /// unreadable state aside (so it isn't overwritten) and starts empty.
    pub fn load_or_default(&self) -> AppState {
        self.load().unwrap_or_else(|e| {
            log::error!("{e}");
            if matches!(e, PersistError::Parse { .. }) {
                match self.backend.move_aside() {
                    Ok(backup) => log::warn!("unreadable state moved to {backup}"),
                    Err(e) => log::error!("could not move unreadable state aside: {e}"),
                }
            }
            AppState::default()
        })
    }

    pub fn save(&self, state: &AppState) -> Result<(), PersistError> {
        let text = toml::to_string_pretty(state)?;
        self.backend.write(&text)
    }
}

/// Desktop: a file in the OS data directory.
#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use std::fs;
    use std::io;
    use std::path::PathBuf;

    use super::PersistError;

    const FILE_NAME: &str = "state.toml";
    const DATA_DIR_ENV: &str = "CRONOSTACK_DATA_DIR";

    #[derive(Debug, Clone)]
    pub struct Backend {
        path: PathBuf,
    }

    impl Backend {
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

        pub fn path(&self) -> &std::path::Path {
            &self.path
        }

        pub fn location(&self) -> String {
            self.path.display().to_string()
        }

        fn error(&self, e: io::Error) -> PersistError {
            PersistError::Storage {
                location: self.location(),
                message: e.to_string(),
            }
        }

        pub fn read(&self) -> Result<Option<String>, PersistError> {
            match fs::read_to_string(&self.path) {
                Ok(text) => Ok(Some(text)),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(self.error(e)),
            }
        }

        /// Writes atomically: to a temporary file first, then renamed over
        /// the real one, so a crash mid-write can't corrupt it.
        pub fn write(&self, text: &str) -> Result<(), PersistError> {
            if let Some(dir) = self.path.parent() {
                fs::create_dir_all(dir).map_err(|e| self.error(e))?;
            }
            let tmp = self.path.with_extension("toml.tmp");
            fs::write(&tmp, text).map_err(|e| self.error(e))?;
            fs::rename(&tmp, &self.path).map_err(|e| self.error(e))
        }

        pub fn move_aside(&self) -> Result<String, PersistError> {
            let backup = self.path.with_extension("toml.broken");
            fs::rename(&self.path, &backup).map_err(|e| self.error(e))?;
            Ok(backup.display().to_string())
        }
    }
}

/// Web: the browser's `localStorage` (kept per site, per browser).
#[cfg(target_arch = "wasm32")]
mod backend {
    use super::PersistError;

    const KEY: &str = "cronostack.state";
    const BROKEN_KEY: &str = "cronostack.state.broken";

    #[derive(Debug, Clone)]
    pub struct Backend;

    impl Backend {
        pub fn locate() -> Self {
            Self
        }

        pub fn location(&self) -> String {
            format!("this browser's local storage (key \"{KEY}\")")
        }

        fn storage(&self) -> Result<web_sys::Storage, PersistError> {
            web_sys::window()
                .and_then(|w| w.local_storage().ok().flatten())
                .ok_or_else(|| PersistError::Storage {
                    location: self.location(),
                    message: "local storage is unavailable (private mode or disabled?)".into(),
                })
        }

        fn error(&self, e: wasm_bindgen::JsValue) -> PersistError {
            PersistError::Storage {
                location: self.location(),
                message: format!("{e:?}"),
            }
        }

        pub fn read(&self) -> Result<Option<String>, PersistError> {
            self.storage()?.get_item(KEY).map_err(|e| self.error(e))
        }

        pub fn write(&self, text: &str) -> Result<(), PersistError> {
            self.storage()?
                .set_item(KEY, text)
                .map_err(|e| self.error(e))
        }

        pub fn move_aside(&self) -> Result<String, PersistError> {
            let storage = self.storage()?;
            if let Some(text) = storage.get_item(KEY).map_err(|e| self.error(e))? {
                storage
                    .set_item(BROKEN_KEY, &text)
                    .map_err(|e| self.error(e))?;
            }
            storage.remove_item(KEY).map_err(|e| self.error(e))?;
            Ok(format!("local storage key \"{BROKEN_KEY}\""))
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Store {
    /// A store in a specific directory (used by `--smoke-test` and tests).
    pub fn in_dir(dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            backend: backend::Backend::in_dir(dir),
        }
    }

    #[cfg(test)]
    fn path(&self) -> &std::path::Path {
        self.backend.path()
    }

    /// The folder holding the state file ("Open folder" in the app).
    pub fn folder(&self) -> Option<&std::path::Path> {
        self.backend.path().parent()
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::fs;

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
