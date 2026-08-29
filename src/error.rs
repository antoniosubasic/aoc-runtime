//! The error type the binary reports.
//!
//! Each module owns a typed error describing what can go wrong inside it; this
//! enum is the union the command handlers return.

use crate::{
    aoc::ApiError, config::ConfigError, env::EnvError, process::ProcessError, resolve::ResolveError,
};
use std::{io, path::Path, path::PathBuf};

/// Anything that can stop a command from completing.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The environment could not be inspected.
    #[error(transparent)]
    Env(#[from] EnvError),

    /// Configuration could not be loaded.
    #[error(transparent)]
    Config(#[from] ConfigError),

    /// Arguments could not be resolved into a plan.
    #[error(transparent)]
    Resolve(#[from] ResolveError),

    /// A child process failed.
    #[error(transparent)]
    Process(#[from] ProcessError),

    /// A request to Advent of Code failed.
    #[error(transparent)]
    Api(#[from] ApiError),

    /// The project directory does not exist yet.
    #[error("project does not exist: {path}\n\ncreate it with `aoc init`")]
    ProjectMissing {
        /// The expected project directory.
        path: PathBuf,
    },

    /// The project directory already exists.
    #[error("project already exists: {path}")]
    ProjectExists {
        /// The existing project directory.
        path: PathBuf,
    },

    /// A destructive action needed confirmation and there was nobody to ask.
    #[error(
        "refusing to remove puzzle inputs and cached answers without confirmation\n\n\
         pass --yes to confirm"
    )]
    ConfirmationRequired,

    /// The puzzle URL could not be handed to a browser.
    #[error("failed to open {url} in a browser")]
    Browser {
        /// The URL that could not be opened.
        url: String,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },

    /// A filesystem operation failed.
    #[error("failed to {action}: {path}")]
    Io {
        /// What was being attempted, phrased as a verb.
        action: &'static str,
        /// The path involved.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
}

/// Attaches the path and the attempted operation to an I/O failure.
pub trait IoResultExt<T> {
    /// Converts an I/O error into [`Error::Io`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] whenever the receiver is an error.
    fn io_context(self, action: &'static str, path: &Path) -> Result<T, Error>;
}

impl<T> IoResultExt<T> for Result<T, io::Error> {
    fn io_context(self, action: &'static str, path: &Path) -> Result<T, Error> {
        self.map_err(|source| Error::Io {
            action,
            path: path.to_path_buf(),
            source,
        })
    }
}

/// Renders an error and its causes to standard error.
pub fn report(error: &Error) {
    use colored::Colorize as _;

    eprintln!("{} {error}", "error:".red().bold());

    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        eprintln!("  {} {cause}", "caused by:".dimmed());
        source = cause.source();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_failures_name_the_action_and_path() {
        let result: Result<(), io::Error> =
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"));

        let error = result
            .io_context(
                "create project directory",
                Path::new("/aoc/2024/day07/rust"),
            )
            .expect_err("the result is an error");

        let message = error.to_string();
        assert!(message.contains("create project directory"), "{message}");
        assert!(message.contains("/aoc/2024/day07/rust"), "{message}");
    }

    #[test]
    fn missing_and_existing_projects_read_clearly() {
        let missing = Error::ProjectMissing {
            path: PathBuf::from("/aoc/2024/day07/rust"),
        };
        let existing = Error::ProjectExists {
            path: PathBuf::from("/aoc/2024/day07/rust"),
        };

        assert!(missing.to_string().contains("aoc init"));
        assert!(existing.to_string().contains("already exists"));
    }
}
