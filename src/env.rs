//! Ambient state: where files live, and what day it is.
//!
//! [`Env::capture`] is the only place the library reads process environment
//! variables or the current directory, and [`Clock`] is the only source of the
//! current date. Both are passed down explicitly so the rest of the crate is
//! deterministic under test.

use chrono::{Datelike, Local, NaiveDate};
use std::{
    env,
    path::{Path, PathBuf},
};

/// The name of the configuration file inside the configuration directory.
pub const CONFIG_FILE_NAME: &str = "config.yaml";

/// Environment variable holding an explicit configuration directory.
pub const CONFIG_DIR_VAR: &str = "AOC_CONFIG_DIR";

/// Environment variable holding the Advent of Code session cookie.
pub const SESSION_VAR: &str = "AOC_SESSION";

/// Where the tool reads and writes, and where it was invoked from.
#[derive(Debug, Clone)]
pub struct Env {
    /// The user's home directory, used to expand a leading `~` in templates.
    pub home: PathBuf,
    /// The directory holding `config.yaml` and the `base` directory.
    pub config_dir: PathBuf,
    /// The configuration file itself. Absolute, even when `--config` named a
    /// relative path.
    pub config_file: PathBuf,
    /// Where cached answers are kept.
    pub state_dir: PathBuf,
    /// The directory the command was invoked from.
    pub cwd: PathBuf,
    /// A session cookie supplied through the environment, which takes
    /// precedence over the one in the configuration file.
    pub session_cookie: Option<String>,
}

impl Env {
    /// Captures the environment, optionally overriding the configuration file.
    ///
    /// The configuration directory is the first of: the parent of an explicit
    /// `--config` file, `$AOC_CONFIG_DIR`, `$XDG_CONFIG_HOME/aoc`, or
    /// `~/.config/aoc`. A relative `--config` path is resolved against the
    /// current directory first, so both it and the directory derived from it
    /// name a real place rather than depending on where the process later
    /// looks from.
    ///
    /// # Errors
    ///
    /// Returns [`EnvError`] if the home or current directory cannot be
    /// determined.
    pub fn capture(config_override: Option<&Path>) -> Result<Self, EnvError> {
        let home = dirs::home_dir().ok_or(EnvError::NoHomeDirectory)?;
        let cwd = env::current_dir().map_err(|source| EnvError::NoCurrentDirectory { source })?;

        let (config_dir, config_file) = config_override.map_or_else(
            || {
                let dir = default_config_dir(&home);
                let file = dir.join(CONFIG_FILE_NAME);
                (dir, file)
            },
            |file| {
                let file = cwd.join(file);
                let dir = file.parent().unwrap_or(cwd.as_path()).to_path_buf();
                (dir, file)
            },
        );

        Ok(Self {
            state_dir: default_state_dir(&home),
            home,
            config_dir,
            config_file,
            cwd,
            session_cookie: non_empty_var(SESSION_VAR),
        })
    }
}

fn default_config_dir(home: &Path) -> PathBuf {
    resolve_config_dir(
        home,
        absolute_dir_var(CONFIG_DIR_VAR).as_deref(),
        absolute_dir_var("XDG_CONFIG_HOME").as_deref(),
    )
}

fn resolve_config_dir(home: &Path, explicit: Option<&Path>, xdg: Option<&Path>) -> PathBuf {
    match (explicit, xdg) {
        (Some(dir), _) => dir.to_path_buf(),
        (None, Some(xdg)) => xdg.join("aoc"),
        (None, None) => home.join(".config").join("aoc"),
    }
}

fn default_state_dir(home: &Path) -> PathBuf {
    resolve_state_dir(home, absolute_dir_var("XDG_STATE_HOME").as_deref())
}

fn resolve_state_dir(home: &Path, xdg: Option<&Path>) -> PathBuf {
    xdg.map_or_else(|| home.join(".local").join("state"), Path::to_path_buf)
        .join("aoc")
}

fn absolute_dir_var(name: &str) -> Option<PathBuf> {
    absolute_dir(env::var(name).ok())
}

fn non_empty_var(name: &str) -> Option<String> {
    non_empty(env::var(name).ok())
}

fn absolute_dir(value: Option<String>) -> Option<PathBuf> {
    let path = PathBuf::from(non_empty(value)?);
    path.is_absolute().then_some(path)
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

/// The source of the current date.
pub trait Clock {
    /// Today's date in the local time zone.
    fn today(&self) -> NaiveDate;
}

/// A clock backed by the system's local time.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn today(&self) -> NaiveDate {
        Local::now().date_naive()
    }
}

/// A clock frozen at a fixed date, for tests and reproducible runs.
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(pub NaiveDate);

impl FixedClock {
    /// Creates a clock frozen at the given calendar date, or `None` if that
    /// date does not exist.
    #[must_use]
    pub fn ymd(year: i32, month: u32, day: u32) -> Option<Self> {
        NaiveDate::from_ymd_opt(year, month, day).map(Self)
    }
}

impl Clock for FixedClock {
    fn today(&self) -> NaiveDate {
        self.0
    }
}

/// Whether a date falls inside an Advent of Code event.
#[must_use]
pub fn is_december(date: NaiveDate) -> bool {
    date.month() == 12
}

/// Errors produced while inspecting the environment.
#[derive(Debug, thiserror::Error)]
pub enum EnvError {
    /// The home directory could not be determined.
    #[error("could not determine the home directory")]
    NoHomeDirectory,
    /// The current directory could not be read.
    #[error("could not determine the current directory")]
    NoCurrentDirectory {
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explicit_config_file_sets_the_directory_to_its_parent() {
        let env = Env::capture(Some(Path::new("/tmp/fixture/config.yaml")))
            .expect("environment should be capturable");

        assert_eq!(env.config_file, Path::new("/tmp/fixture/config.yaml"));
        assert_eq!(env.config_dir, Path::new("/tmp/fixture"));
    }

    #[test]
    fn a_bare_config_file_name_resolves_against_the_current_directory() {
        let env =
            Env::capture(Some(Path::new("config.yaml"))).expect("environment should be capturable");

        assert_eq!(env.config_dir, env.cwd);
        assert_eq!(env.config_file, env.cwd.join("config.yaml"));
    }

    #[test]
    fn the_config_directory_falls_back_to_dot_config_aoc() {
        let home = Path::new("/home/tester");

        assert_eq!(
            resolve_config_dir(home, None, None),
            Path::new("/home/tester/.config/aoc")
        );
        assert_eq!(
            resolve_config_dir(home, None, Some(Path::new("/xdg"))),
            Path::new("/xdg/aoc")
        );
        assert_eq!(
            resolve_config_dir(home, Some(Path::new("/explicit")), Some(Path::new("/xdg"))),
            Path::new("/explicit")
        );
    }

    #[test]
    fn the_state_directory_follows_xdg_when_set() {
        let home = Path::new("/home/tester");

        assert_eq!(
            resolve_state_dir(home, None),
            Path::new("/home/tester/.local/state/aoc")
        );
        assert_eq!(
            resolve_state_dir(home, Some(Path::new("/xdg"))),
            Path::new("/xdg/aoc")
        );
    }

    #[test]
    fn relative_directory_variables_are_ignored() {
        // A leading slash names the root of the current drive on Windows, which
        // is not an absolute path there.
        let absolute = if cfg!(windows) {
            r"C:\absolute\path"
        } else {
            "/absolute/path"
        };

        assert_eq!(absolute_dir(Some("relative/path".to_owned())), None);
        assert_eq!(
            absolute_dir(Some(absolute.to_owned())),
            Some(PathBuf::from(absolute))
        );
        assert_eq!(absolute_dir(None), None);
    }

    #[test]
    fn blank_variables_are_treated_as_unset() {
        assert_eq!(non_empty(Some("   ".to_owned())), None);
        assert_eq!(non_empty(Some(String::new())), None);
        assert_eq!(
            non_empty(Some("value".to_owned())),
            Some("value".to_owned())
        );
    }

    #[test]
    fn december_is_recognised() {
        let december = FixedClock::ymd(2024, 12, 1).expect("valid date");
        let november = FixedClock::ymd(2024, 11, 30).expect("valid date");

        assert!(is_december(december.today()));
        assert!(!is_december(november.today()));
    }
}
