//! `aoc init`: create the project directory and scaffold a solution in it.

use super::App;
use crate::{
    error::{Error, IoResultExt as _},
    language::Language,
    process::IoPolicy,
    puzzle::Puzzle,
};
use std::{fs, path::Path};

impl App<'_> {
    pub(super) fn init(
        &mut self,
        puzzle: Puzzle,
        language: Language,
        project: &Path,
    ) -> Result<(), Error> {
        if project.exists() {
            return Err(Error::ProjectExists {
                path: project.to_path_buf(),
            });
        }

        fs::create_dir_all(project).io_context("create project directory", project)?;

        match self.scaffold(language, project) {
            Ok(()) => {
                self.ensure_input(puzzle, project);
                Ok(())
            }
            Err(error) => {
                let _ = fs::remove_dir_all(project);
                Err(error)
            }
        }
    }

    fn scaffold(&mut self, language: Language, project: &Path) -> Result<(), Error> {
        let commands = language.commands(project)?;

        if let Some(init) = &commands.init {
            self.runner.run_checked(init, IoPolicy::INHERIT)?;
        } else {
            let entry = language.entry_file(project);
            create_parent(&entry)?;
            fs::File::create(&entry).io_context("create solution file", &entry)?;
        }

        self.copy_base_file(language, project)
    }

    fn copy_base_file(&mut self, language: Language, project: &Path) -> Result<(), Error> {
        let base = language.base_file(&self.config.config_dir);
        if !base.exists() {
            return Ok(());
        }

        let entry = language.entry_file(project);
        create_parent(&entry)?;
        fs::copy(&base, &entry).io_context("copy base file", &base)?;

        Ok(())
    }
}

fn create_parent(path: &Path) -> Result<(), Error> {
    match path.parent() {
        Some(parent) => fs::create_dir_all(parent).io_context("create directory", parent),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aoc::fake::FakeClient,
        app::{INPUT_FILE_NAME, testing::Harness},
        puzzle::{Day, Year},
    };

    fn puzzle() -> Puzzle {
        Puzzle::new(
            Year::new(2024).expect("valid year"),
            Day::new(7).expect("valid day"),
        )
        .expect("2024 has a day 7")
    }

    #[test]
    fn creates_the_entry_file_for_languages_without_a_scaffolding_tool() {
        let root = tempfile::tempdir().expect("temp dir");
        let project = root.path().join("2024").join("day07").join("python");
        let mut harness = Harness::new(root.path());

        harness
            .app()
            .init(puzzle(), Language::Python, &project)
            .expect("init should succeed");

        assert!(project.join("main.py").is_file());
        assert!(
            harness.runner.executed().is_empty(),
            "python needs no scaffolding process"
        );
    }

    #[test]
    fn delegates_to_the_language_toolchain_when_there_is_one() {
        let root = tempfile::tempdir().expect("temp dir");
        let project = root.path().join("2024").join("day07").join("rust");
        let mut harness = Harness::new(root.path());

        harness
            .app()
            .init(puzzle(), Language::Rust, &project)
            .expect("init should succeed");

        let executed = harness.runner.executed();
        assert_eq!(executed.len(), 1);
        assert_eq!(executed[0].program(), "cargo");
    }

    #[test]
    fn copies_the_base_file_over_the_entry_point() {
        let root = tempfile::tempdir().expect("temp dir");
        let project = root.path().join("2024").join("day07").join("python");
        let mut harness = Harness::new(root.path());

        let base = Language::Python.base_file(&harness.config.config_dir);
        fs::create_dir_all(base.parent().expect("base directory")).expect("create base dir");
        fs::write(&base, "print('hi')\n").expect("write base file");

        harness
            .app()
            .init(puzzle(), Language::Python, &project)
            .expect("init should succeed");

        assert_eq!(
            fs::read_to_string(project.join("main.py")).expect("entry file"),
            "print('hi')\n"
        );
    }

    #[test]
    fn an_absent_base_file_leaves_an_empty_entry_point() {
        let root = tempfile::tempdir().expect("temp dir");
        let project = root.path().join("2024").join("day07").join("java");
        let mut harness = Harness::new(root.path());

        harness
            .app()
            .init(puzzle(), Language::Java, &project)
            .expect("init should succeed");

        assert_eq!(
            fs::read_to_string(project.join("Main.java")).expect("entry file"),
            ""
        );
    }

    #[test]
    fn an_existing_project_is_an_error() {
        let root = tempfile::tempdir().expect("temp dir");
        let project = root.path().join("2024").join("day07").join("python");
        fs::create_dir_all(&project).expect("create project");
        let mut harness = Harness::new(root.path());

        let error = harness
            .app()
            .init(puzzle(), Language::Python, &project)
            .expect_err("project already exists");

        assert!(matches!(error, Error::ProjectExists { .. }), "{error:?}");
    }

    #[test]
    fn a_failed_scaffold_leaves_nothing_behind() {
        let root = tempfile::tempdir().expect("temp dir");
        let project = root.path().join("2024").join("day07").join("rust");
        let mut harness = Harness::new(root.path());
        harness.runner.push_failure(1, "cargo init exploded");

        let error = harness
            .app()
            .init(puzzle(), Language::Rust, &project)
            .expect_err("scaffolding failed");

        assert!(error.to_string().contains("cargo init exploded"), "{error}");
        assert!(
            !project.exists(),
            "a half-created project would block the next `aoc init`"
        );
    }

    #[test]
    fn downloads_the_input_for_a_new_project() {
        let root = tempfile::tempdir().expect("temp dir");
        let project = root.path().join("2024").join("day07").join("python");
        let mut harness =
            Harness::new(root.path()).with_client(FakeClient::with_input("puzzle input"));

        harness
            .app()
            .init(puzzle(), Language::Python, &project)
            .expect("init should succeed");

        let input = root.path().join("2024").join("day07").join(INPUT_FILE_NAME);
        assert_eq!(fs::read_to_string(input).expect("input"), "puzzle input");
    }
}
