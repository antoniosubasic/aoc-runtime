//! Keeping puzzle inputs for good.
//!
//! An input is personal, permanent and unchanging, so it is worth downloading
//! exactly once. The copy that matters lives in the state directory; the
//! `input.txt` a solution reads beside its project is a symbolic link pointing
//! at it. Scaffolding a day again, moving the solutions tree or deleting a
//! project therefore costs a link rather than another request to the site.

use crate::{
    error::{Error, IoResultExt as _},
    puzzle::Puzzle,
    store,
};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Puzzle inputs, one file per puzzle under the state directory.
#[derive(Debug, Clone)]
pub struct InputStore {
    root: PathBuf,
}

impl InputStore {
    /// Creates a store rooted at the given state directory.
    #[must_use]
    pub fn new(state_dir: impl Into<PathBuf>) -> Self {
        Self {
            root: state_dir.into().join("inputs"),
        }
    }

    /// Where a puzzle's input is kept, whether or not it has been downloaded.
    #[must_use]
    pub fn path(&self, puzzle: Puzzle) -> PathBuf {
        self.root.join(format!("{}.txt", puzzle.slug()))
    }

    /// Whether this puzzle's input has already been downloaded.
    #[must_use]
    pub fn holds(&self, puzzle: Puzzle) -> bool {
        self.path(puzzle).is_file()
    }

    /// How many inputs are stored.
    ///
    /// Best effort, like the rest of the store's reads: a directory that
    /// cannot be listed counts as empty.
    #[must_use]
    pub fn count(&self) -> usize {
        fs::read_dir(&self.root).map_or(0, |entries| entries.flatten().count())
    }

    /// Removes every stored input.
    ///
    /// Unlike everything else in the state directory this cannot be
    /// regenerated for free - each input costs another request to the site -
    /// so only an explicit instruction from the user should reach here.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the directory exists and cannot be removed.
    pub fn clear(&self) -> Result<(), Error> {
        store::remove_tree(&self.root, "remove cached inputs")
    }

    /// Writes a freshly downloaded input to the store.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the state directory cannot be written to.
    pub fn store(&self, puzzle: Puzzle, text: &str) -> Result<(), Error> {
        fs::create_dir_all(&self.root).io_context("create input cache directory", &self.root)?;

        let path = self.path(puzzle);
        fs::write(&path, text).io_context("cache puzzle input", &path)
    }

    /// Points `at` to this puzzle's stored input, which [`InputStore::holds`]
    /// must already report as present.
    ///
    /// Anything already sitting at `at` is a link left over from an input the
    /// store no longer has, so it is replaced rather than treated as an
    /// obstacle: a state directory that was wiped heals on the next run.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the link cannot be created and the input cannot
    /// be copied into its place either.
    pub fn link(&self, puzzle: Puzzle, at: &Path) -> Result<(), Error> {
        let target = self.path(puzzle);

        if let Some(parent) = at.parent() {
            fs::create_dir_all(parent).io_context("create input directory", parent)?;
        }

        if fs::symlink_metadata(at).is_ok() {
            fs::remove_file(at).io_context("replace stale input link", at)?;
        }

        // Windows only lets an unprivileged process create symbolic links in
        // developer mode, so a copy stands in where linking is refused. The
        // download is still saved once and only once, which is the point.
        if symlink(&target, at).is_err() {
            fs::copy(&target, at)
                .map(drop)
                .io_context("link cached input", at)?;
        }

        Ok(())
    }
}

/// Creates a symbolic link at `at` pointing to `target`.
#[cfg(unix)]
fn symlink(target: &Path, at: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}

/// Creates a symbolic link at `at` pointing to `target`.
#[cfg(windows)]
fn symlink(target: &Path, at: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(target, at)
}

/// Creates a symbolic link at `at` pointing to `target`.
#[cfg(not(any(unix, windows)))]
fn symlink(_target: &Path, _at: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "symbolic links are not supported on this platform",
    ))
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
    fn an_input_survives_the_store_being_reopened() {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = InputStore::new(dir.path());

        assert!(!store.holds(puzzle()));
        store.store(puzzle(), "puzzle input").expect("store input");

        let reopened = InputStore::new(dir.path());
        assert!(reopened.holds(puzzle()));
        assert_eq!(
            fs::read_to_string(reopened.path(puzzle())).expect("read stored input"),
            "puzzle input"
        );
    }

    #[test]
    fn puzzles_are_kept_apart() {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = InputStore::new(dir.path());
        let other = Puzzle::new(
            Year::new(2024).expect("valid year"),
            Day::new(17).expect("valid day"),
        )
        .expect("2024 has a day 17");

        store.store(puzzle(), "seven").expect("store input");

        assert!(store.holds(puzzle()));
        assert!(!store.holds(other), "day 7 must not answer for day 17");
    }

    #[test]
    fn a_linked_input_reads_as_the_stored_one() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project = tempfile::tempdir().expect("temp dir");
        let store = InputStore::new(dir.path());
        store.store(puzzle(), "puzzle input").expect("store input");

        let at = project.path().join("day07").join("input.txt");
        store.link(puzzle(), &at).expect("link input");

        assert_eq!(
            fs::read_to_string(&at).expect("read linked input"),
            "puzzle input"
        );
    }

    #[cfg(unix)]
    #[test]
    fn linking_points_at_the_cache_rather_than_copying_it() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project = tempfile::tempdir().expect("temp dir");
        let store = InputStore::new(dir.path());
        store.store(puzzle(), "puzzle input").expect("store input");

        let at = project.path().join("input.txt");
        store.link(puzzle(), &at).expect("link input");

        assert!(
            fs::symlink_metadata(&at)
                .expect("the link exists")
                .is_symlink()
        );
        assert_eq!(fs::read_link(&at).expect("read link"), store.path(puzzle()));
    }

    // A state directory that was cleared leaves every project pointing at
    // nothing. The next run downloads the input again, and the link it writes
    // has to survive meeting the dead one. Dangling links are the premise, so
    // this is about the platforms that link rather than copy.
    #[cfg(unix)]
    #[test]
    fn a_link_left_over_from_a_wiped_cache_is_replaced() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project = tempfile::tempdir().expect("temp dir");
        let store = InputStore::new(dir.path());
        let at = project.path().join("input.txt");

        store.store(puzzle(), "first").expect("store input");
        store.link(puzzle(), &at).expect("link input");

        fs::remove_dir_all(dir.path()).expect("wipe the state directory");
        assert!(!at.exists(), "the link now points at nothing");

        store.store(puzzle(), "second").expect("store input again");
        store.link(puzzle(), &at).expect("relink input");

        assert_eq!(
            fs::read_to_string(&at).expect("read linked input"),
            "second"
        );
    }

    // Whatever is already at the link's place gives way, whether this platform
    // got there by linking or by copying.
    #[test]
    fn linking_replaces_whatever_is_already_there() {
        let dir = tempfile::tempdir().expect("temp dir");
        let project = tempfile::tempdir().expect("temp dir");
        let store = InputStore::new(dir.path());
        let at = project.path().join("input.txt");

        store.store(puzzle(), "first").expect("store input");
        store.link(puzzle(), &at).expect("link input");

        store.store(puzzle(), "second").expect("store input again");
        store.link(puzzle(), &at).expect("relink input");

        assert_eq!(
            fs::read_to_string(&at).expect("read linked input"),
            "second"
        );
    }

    // A file where the state directory should be is the one way to make the
    // store unwritable that means the same thing on every platform.
    #[test]
    fn an_unwritable_location_is_reported_rather_than_ignored() {
        let dir = tempfile::tempdir().expect("temp dir");
        let blocked = dir.path().join("state");
        fs::write(&blocked, "a file, not a directory").expect("block the state directory");

        let error = InputStore::new(&blocked)
            .store(puzzle(), "puzzle input")
            .expect_err("the state directory cannot be written to");

        assert!(matches!(error, Error::Io { .. }), "{error:?}");
    }

    #[test]
    fn clearing_removes_every_stored_input() {
        let dir = tempfile::tempdir().expect("temp dir");
        let inputs = InputStore::new(dir.path());
        inputs.store(puzzle(), "1227\n").expect("store the input");

        assert_eq!(inputs.count(), 1);
        inputs.clear().expect("clearing should succeed");

        assert_eq!(inputs.count(), 0);
        assert!(!inputs.holds(puzzle()), "the input survived");
        assert!(
            dir.path().exists(),
            "the state directory itself must remain"
        );
    }

    #[test]
    fn an_empty_store_counts_nothing() {
        let dir = tempfile::tempdir().expect("temp dir");

        assert_eq!(InputStore::new(dir.path()).count(), 0);
    }
}
