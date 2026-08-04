//! Remembering answers that were already accepted.
//!
//! Without this, every `aoc run` re-submits part one even when it was solved
//! days ago, which costs two HTTP round trips and needlessly loads the site.
//! Caching is strictly best effort: a failure to read or write never fails a
//! run.

use crate::puzzle::{Part, Puzzle};
use std::{fs, path::PathBuf};

/// Stores answers known to be correct.
pub trait AnswerCache {
    /// The accepted answer for this part, if one is known.
    fn correct(&self, puzzle: Puzzle, part: Part) -> Option<String>;

    /// Records an accepted answer. Failures are silently ignored.
    fn record(&self, puzzle: Puzzle, part: Part, answer: &str);
}

/// A cache backed by one small file per answer.
#[derive(Debug, Clone)]
pub struct FileCache {
    root: PathBuf,
}

impl FileCache {
    /// Creates a cache rooted at the given state directory.
    #[must_use]
    pub fn new(state_dir: impl Into<PathBuf>) -> Self {
        Self {
            root: state_dir.into().join("answers"),
        }
    }

    fn path(&self, puzzle: Puzzle, part: Part) -> PathBuf {
        self.root.join(format!(
            "{}-{:02}-part{}",
            puzzle.year.get(),
            puzzle.day.get(),
            part.number()
        ))
    }
}

impl AnswerCache for FileCache {
    fn correct(&self, puzzle: Puzzle, part: Part) -> Option<String> {
        let answer = fs::read_to_string(self.path(puzzle, part)).ok()?;
        let answer = answer.trim().to_owned();

        (!answer.is_empty()).then_some(answer)
    }

    fn record(&self, puzzle: Puzzle, part: Part, answer: &str) {
        let path = self.path(puzzle, part);
        if fs::create_dir_all(&self.root).is_ok() {
            let _ = fs::write(path, answer);
        }
    }
}

#[cfg(test)]
pub(crate) mod memory {
    use super::{AnswerCache, Part, Puzzle};
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[derive(Debug, Default)]
    pub(crate) struct MemoryCache {
        entries: RefCell<HashMap<(Puzzle, Part), String>>,
    }

    impl MemoryCache {
        pub(crate) fn new() -> Self {
            Self::default()
        }

        pub(crate) fn seeded(puzzle: Puzzle, part: Part, answer: &str) -> Self {
            let cache = Self::new();
            cache.record(puzzle, part, answer);
            cache
        }
    }

    impl AnswerCache for MemoryCache {
        fn correct(&self, puzzle: Puzzle, part: Part) -> Option<String> {
            self.entries.borrow().get(&(puzzle, part)).cloned()
        }

        fn record(&self, puzzle: Puzzle, part: Part, answer: &str) {
            self.entries
                .borrow_mut()
                .insert((puzzle, part), answer.to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::puzzle::{Day, Year};

    fn puzzle() -> Puzzle {
        Puzzle::new(
            Year::new(2024).expect("valid year"),
            Day::new(7).expect("valid day"),
        )
        .expect("2024 has a day 7")
    }

    #[test]
    fn remembers_answers_across_instances() {
        let dir = tempfile::tempdir().expect("temp dir");
        let cache = FileCache::new(dir.path());

        assert_eq!(cache.correct(puzzle(), Part::One), None);
        cache.record(puzzle(), Part::One, "1227");

        let reopened = FileCache::new(dir.path());
        assert_eq!(
            reopened.correct(puzzle(), Part::One).as_deref(),
            Some("1227")
        );
        assert_eq!(reopened.correct(puzzle(), Part::Two), None);
    }

    #[test]
    fn keeps_parts_and_puzzles_apart() {
        let dir = tempfile::tempdir().expect("temp dir");
        let cache = FileCache::new(dir.path());
        let other = Puzzle::new(
            Year::new(2023).expect("valid year"),
            Day::new(7).expect("valid day"),
        )
        .expect("2023 has a day 7");

        cache.record(puzzle(), Part::One, "a");
        cache.record(puzzle(), Part::Two, "b");
        cache.record(other, Part::One, "c");

        assert_eq!(cache.correct(puzzle(), Part::One).as_deref(), Some("a"));
        assert_eq!(cache.correct(puzzle(), Part::Two).as_deref(), Some("b"));
        assert_eq!(cache.correct(other, Part::One).as_deref(), Some("c"));
    }

    #[test]
    fn an_unwritable_location_is_not_an_error() {
        let dir = tempfile::tempdir().expect("temp dir");
        let blocked = dir.path().join("state");
        fs::write(&blocked, "a file, not a directory").expect("block the state directory");

        let cache = FileCache::new(&blocked);

        cache.record(puzzle(), Part::One, "1227");
        assert_eq!(cache.correct(puzzle(), Part::One), None);
    }
}
