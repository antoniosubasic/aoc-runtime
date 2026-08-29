//! Where compiled solutions are kept.
//!
//! A build artifact is regenerable, uninteresting to the user and specific to
//! one puzzle and language, so it belongs in the state directory rather than in
//! the solutions tree: a project holds sources, and nothing a compiler wrote.
//! Directories are named after the puzzle exactly as
//! [`InputStore`](crate::aoc::input::InputStore) names inputs.

use crate::{language::Language, puzzle::Puzzle};
use std::path::{Path, PathBuf};

/// The single file a directly compiled language builds to.
pub const BINARY_NAME: &str = "bin";

/// Compiled solutions, one directory per puzzle and language under the state
/// directory.
#[derive(Debug, Clone)]
pub struct BuildStore {
    root: PathBuf,
}

impl BuildStore {
    /// Creates a store rooted at the given state directory.
    #[must_use]
    pub fn new(state_dir: impl Into<PathBuf>) -> Self {
        Self {
            root: state_dir.into().join("builds"),
        }
    }

    /// Where this puzzle's solution in this language builds to, whether or not
    /// the directory exists yet. Languages driven by their own build tool are
    /// pointed at it and lay it out as they please; the rest write a single
    /// [`BINARY_NAME`] file into it.
    #[must_use]
    pub fn dir(&self, puzzle: Puzzle, language: Language) -> PathBuf {
        self.root.join(puzzle.slug()).join(language.name())
    }
}

/// Where a directly compiled language's binary lands inside its build
/// directory.
#[must_use]
pub fn binary(artifacts: &Path) -> PathBuf {
    artifacts.join(BINARY_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aoc::input::InputStore,
        puzzle::{Day, Year},
    };

    fn puzzle() -> Puzzle {
        Puzzle::new(
            Year::new(2024).expect("valid year"),
            Day::new(7).expect("valid day"),
        )
        .expect("2024 ran 25 days")
    }

    #[test]
    fn a_build_directory_is_named_after_its_puzzle_and_language() {
        let builds = BuildStore::new("/state");

        assert_eq!(
            builds.dir(puzzle(), Language::Rust),
            Path::new("/state/builds/2024-07/rust")
        );
    }

    #[test]
    fn languages_of_the_same_puzzle_do_not_share_a_directory() {
        let builds = BuildStore::new("/state");

        assert_ne!(
            builds.dir(puzzle(), Language::Rust),
            builds.dir(puzzle(), Language::Java)
        );
    }

    #[test]
    fn builds_and_inputs_agree_on_the_name_of_a_puzzle() {
        let builds = BuildStore::new("/state");
        let inputs = InputStore::new("/state");

        let build_dir = builds.dir(puzzle(), Language::Rust);
        let stem = inputs
            .path(puzzle())
            .file_stem()
            .expect("an input path names a file")
            .to_os_string();

        assert_eq!(
            build_dir
                .parent()
                .and_then(Path::file_name)
                .expect("a build directory sits under its puzzle"),
            stem
        );
    }

    #[test]
    fn the_binary_sits_directly_inside_the_build_directory() {
        assert_eq!(
            binary(Path::new("/state/builds/2024-07/c")),
            Path::new("/state/builds/2024-07/c/bin")
        );
    }
}
