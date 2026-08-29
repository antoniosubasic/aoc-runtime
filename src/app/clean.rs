//! Emptying the state directory.
//!
//! Build output is regenerable, so removing it costs nothing and is never
//! questioned. Inputs and answers are not: an input costs another request to
//! the site and an answer costs a submission, so removing those is confirmed
//! first and only ever because the user asked for it.
//!
//! The last-request stamp is deliberately not among the things removed. It
//! sits at the root of the state directory rather than inside any of the three
//! caches, so the throttle keeps holding across invocations without this
//! handler having to say anything about it.

use crate::{app::App, error::Error};

impl App<'_> {
    /// Removes build output, and with `all` the cached inputs and answers too.
    ///
    /// Returns without removing anything if the user declines; that is an
    /// answer, not a failure, so the modes after this one still run.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ConfirmationRequired`] if `all` is set and there is
    /// nobody to ask, or [`Error::Io`] if something cannot be removed.
    pub(super) fn clean(&mut self, all: bool) -> Result<(), Error> {
        if !all {
            return self.builds.clear();
        }

        let count = self.inputs.count();
        let question = format!(
            "remove all build output, {count} downloaded {} and every cached answer? \
             the inputs will be downloaded again on the next run",
            if count == 1 { "input" } else { "inputs" },
        );

        if !self.confirm.confirm(&question)? {
            return Ok(());
        }

        self.builds.clear()?;
        self.inputs.clear()?;
        self.cache.clear()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        aoc::{cache::AnswerCache as _, throttle::LAST_REQUEST_FILE},
        app::testing::Harness,
        error::Error,
        language::Language,
        puzzle::{Day, Part, Puzzle, Year},
        report::scripted::ScriptedConfirm,
        resolve::Plan,
    };
    use std::{fs, path::Path};

    fn puzzle() -> Puzzle {
        Puzzle::new(
            Year::new(2024).expect("valid year"),
            Day::new(7).expect("valid day"),
        )
        .expect("2024 ran 25 days")
    }

    /// Fills the state directory with one of everything and returns the stamp
    /// that must survive whatever happens next.
    fn seed(harness: &Harness, state: &Path) -> std::path::PathBuf {
        let build = harness.builds.dir(puzzle(), Language::Go);
        fs::create_dir_all(&build).expect("create a build directory");
        fs::write(build.join("bin"), "a binary").expect("write a binary");

        harness
            .inputs
            .store(puzzle(), "1227\n")
            .expect("store an input");
        harness.cache.record(puzzle(), Part::One, "1227");

        let stamp = state.join(LAST_REQUEST_FILE);
        fs::write(&stamp, "99000000000000").expect("seed the request stamp");

        stamp
    }

    #[test]
    fn a_bare_clean_removes_only_build_output() {
        let root = tempfile::tempdir().expect("temp dir");
        let state = root.path().join("state");
        let mut harness = Harness::new(root.path());
        let stamp = seed(&harness, &state);

        harness
            .app()
            .execute(Plan::Clean { all: false })
            .expect("cleaning should succeed");

        assert!(!state.join("builds").exists(), "build output survived");
        assert!(harness.inputs.holds(puzzle()), "the input was removed");
        assert_eq!(
            harness.cache.correct(puzzle(), Part::One).as_deref(),
            Some("1227"),
            "the answer was forgotten"
        );
        assert!(stamp.is_file(), "the request stamp was removed");
        assert!(
            harness.confirm.questions.is_empty(),
            "removing build output needs no confirmation"
        );
    }

    #[test]
    fn clean_all_removes_inputs_and_answers() {
        let root = tempfile::tempdir().expect("temp dir");
        let state = root.path().join("state");
        let mut harness = Harness::new(root.path()).with_confirm(ScriptedConfirm::approving());
        let stamp = seed(&harness, &state);

        harness
            .app()
            .execute(Plan::Clean { all: true })
            .expect("cleaning should succeed");

        assert!(!state.join("builds").exists(), "build output survived");
        assert!(!harness.inputs.holds(puzzle()), "the input survived");
        assert_eq!(harness.cache.correct(puzzle(), Part::One), None);
        assert!(stamp.is_file(), "the request stamp was removed");
    }

    #[test]
    fn the_prompt_says_how_many_inputs_are_at_stake() {
        let root = tempfile::tempdir().expect("temp dir");
        let state = root.path().join("state");
        let mut harness = Harness::new(root.path()).with_confirm(ScriptedConfirm::approving());
        seed(&harness, &state);

        harness
            .app()
            .execute(Plan::Clean { all: true })
            .expect("cleaning should succeed");

        let question = harness.confirm.questions.first().expect("one question");
        assert!(question.contains("1 downloaded input"), "{question}");
        assert!(question.contains("downloaded again"), "{question}");
    }

    #[test]
    fn a_declined_clean_removes_nothing() {
        let root = tempfile::tempdir().expect("temp dir");
        let state = root.path().join("state");
        let mut harness = Harness::new(root.path()).with_confirm(ScriptedConfirm::declining());
        seed(&harness, &state);

        harness
            .app()
            .execute(Plan::Clean { all: true })
            .expect("declining is an answer, not a failure");

        assert!(state.join("builds").exists(), "build output was removed");
        assert!(harness.inputs.holds(puzzle()), "the input was removed");
        assert_eq!(
            harness.cache.correct(puzzle(), Part::One).as_deref(),
            Some("1227")
        );
    }

    #[test]
    fn an_unattended_clean_all_is_refused() {
        let root = tempfile::tempdir().expect("temp dir");
        let state = root.path().join("state");
        let mut harness = Harness::new(root.path());
        seed(&harness, &state);

        let error = harness
            .app()
            .execute(Plan::Clean { all: true })
            .expect_err("there is nobody to confirm with");

        assert!(matches!(error, Error::ConfirmationRequired), "{error:?}");
        assert!(harness.inputs.holds(puzzle()), "the input was removed");
    }

    #[test]
    fn cleaning_a_state_directory_that_was_never_used_is_not_an_error() {
        let root = tempfile::tempdir().expect("temp dir");
        let mut harness = Harness::new(root.path()).with_confirm(ScriptedConfirm::approving());

        harness
            .app()
            .execute(Plan::Clean { all: true })
            .expect("nothing to remove is not a failure");
    }
}
