//! What the directories under the state directory have in common.
//!
//! Each store owns its own root and names its own files - that is the whole
//! point of keeping them apart - but they empty themselves the same way, and
//! the rule that makes emptying safe is written down once, here.

use crate::error::{Error, IoResultExt as _};
use std::{fs, io, path::Path};

/// Removes `root` and everything under it.
///
/// A root that was never created is already empty, so its absence is success
/// rather than something for the user to read about.
///
/// # Errors
///
/// Returns [`Error::Io`], phrased with `action`, if the tree exists and cannot
/// be removed.
pub(crate) fn remove_tree(root: &Path, action: &'static str) -> Result<(), Error> {
    match fs::remove_dir_all(root) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result.io_context(action, root),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removing_a_tree_takes_everything_under_it() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("store");
        let nested = root.join("nested");
        fs::create_dir_all(&nested).expect("create a nested directory");
        fs::write(nested.join("file"), "content").expect("write a file");

        remove_tree(&root, "remove the store").expect("removal should succeed");

        assert!(!root.exists(), "the tree survived");
        assert!(dir.path().exists(), "only the store's own root goes");
    }

    #[test]
    fn removing_a_tree_that_was_never_created_is_not_an_error() {
        let dir = tempfile::tempdir().expect("temp dir");

        remove_tree(&dir.path().join("never-used"), "remove nothing")
            .expect("nothing to remove is not a failure");
    }
}
