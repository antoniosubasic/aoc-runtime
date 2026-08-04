//! Executing a resolved [`Plan`].
//!
//! Every outside dependency is injected, so each handler can be driven end to
//! end in a test with a fake runner, a fake client and a temporary directory.

mod init;
mod run;

use crate::{
    aoc::{AocClient, cache::AnswerCache, input::InputStore},
    config::Config,
    error::Error,
    process::{CommandRunner, CommandSpec},
    puzzle::Puzzle,
    report::Reporter,
    resolve::Plan,
};
use std::path::Path;

/// The file a solution reads its puzzle input from.
pub const INPUT_FILE_NAME: &str = "input.txt";

/// Everything a command handler needs.
pub struct App<'a> {
    /// Validated configuration.
    pub config: &'a Config,
    /// How child processes are executed.
    pub runner: &'a dyn CommandRunner,
    /// The Advent of Code client, absent when no session cookie is configured.
    pub client: Option<&'a dyn AocClient>,
    /// Where accepted answers are remembered.
    pub cache: &'a dyn AnswerCache,
    /// Where downloaded puzzle inputs are kept. Concrete rather than a trait,
    /// unlike its neighbours: linking a project at a cached file is what it
    /// does, and there is no linking without a filesystem to do it on.
    pub inputs: &'a InputStore,
    /// Where output goes.
    pub reporter: &'a mut dyn Reporter,
}

impl std::fmt::Debug for App<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("config", &self.config)
            .field("has_client", &self.client.is_some())
            .finish_non_exhaustive()
    }
}

impl App<'_> {
    /// Carries out a plan.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if the work cannot be completed.
    pub fn execute(&mut self, plan: Plan) -> Result<(), Error> {
        match plan {
            Plan::Run {
                puzzle,
                language,
                project,
                submit,
            } => self.run(puzzle, language, &project, submit),
            Plan::Init {
                puzzle,
                language,
                project,
            } => self.init(puzzle, language, &project),
            Plan::Path { project } => {
                self.reporter.data(&project.to_string_lossy());
                Ok(())
            }
            Plan::Code { project } => self.open_editor(&project),
            Plan::Url { puzzle } => {
                self.reporter.data(&puzzle.url());
                Ok(())
            }
        }
    }

    fn open_editor(&mut self, project: &Path) -> Result<(), Error> {
        if !project.exists() {
            return Err(Error::ProjectMissing {
                path: project.to_path_buf(),
            });
        }

        let editor = CommandSpec::new(&self.config.editor).arg(project);
        self.runner.spawn_detached(&editor)?;
        Ok(())
    }

    /// Makes sure a solution has its `input.txt` to read.
    ///
    /// The input itself lives in the state directory and the file beside the
    /// project is a link to it, so a day is downloaded once and never again:
    /// a project that lost its `input.txt`, or never had one, is linked at the
    /// copy already on disk instead of costing another request.
    fn ensure_input(&mut self, puzzle: Puzzle, project: &Path) {
        let Some(parent) = project.parent() else {
            self.reporter
                .warn("project path has no parent directory, skipping input download");
            return;
        };

        let input = parent.join(INPUT_FILE_NAME);
        if input.exists() {
            return;
        }

        if !self.inputs.holds(puzzle) {
            let Some(client) = self.client else {
                self.reporter.warn(&format!(
                    "no session cookie configured, so {} was not downloaded",
                    input.display()
                ));
                return;
            };

            let downloaded = client
                .fetch_input(puzzle)
                .map_err(Error::from)
                .and_then(|text| self.inputs.store(puzzle, &text));

            if let Err(error) = downloaded {
                self.reporter
                    .warn(&format!("could not download puzzle input: {error}"));
                return;
            }
        }

        if let Err(error) = self.inputs.link(puzzle, &input) {
            self.reporter
                .warn(&format!("could not link puzzle input: {error}"));
        }
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::App;
    use crate::{
        aoc::{cache::memory::MemoryCache, fake::FakeClient, input::InputStore},
        config::Config,
        env::Env,
        process::fake::FakeRunner,
        report::recording::RecordingReporter,
    };
    use std::path::Path;

    pub(crate) struct Harness {
        pub(crate) config: Config,
        pub(crate) runner: FakeRunner,
        pub(crate) client: Option<FakeClient>,
        pub(crate) cache: MemoryCache,
        pub(crate) inputs: InputStore,
        pub(crate) reporter: RecordingReporter,
    }

    impl Harness {
        pub(crate) fn new(root: &Path) -> Self {
            Self {
                config: config_for(root),
                runner: FakeRunner::new(),
                client: None,
                cache: MemoryCache::new(),
                inputs: InputStore::new(root.join("state")),
                reporter: RecordingReporter::new(),
            }
        }

        pub(crate) fn with_client(mut self, client: FakeClient) -> Self {
            self.client = Some(client);
            self
        }

        pub(crate) fn with_cache(mut self, cache: MemoryCache) -> Self {
            self.cache = cache;
            self
        }

        pub(crate) fn app(&mut self) -> App<'_> {
            App {
                config: &self.config,
                runner: &self.runner,
                client: self.client.as_ref().map(|client| client as _),
                cache: &self.cache,
                inputs: &self.inputs,
                reporter: &mut self.reporter,
            }
        }
    }

    fn config_for(root: &Path) -> Config {
        let env = Env {
            home: root.to_path_buf(),
            config_dir: root.join("config"),
            config_file: root.join("config").join("config.yaml"),
            state_dir: root.join("state"),
            cwd: root.to_path_buf(),
            session_cookie: None,
        };

        let yaml = format!(
            "template_path: \"{}/{{{{year}}}}/day{{{{pad day}}}}/{{{{language}}}}\"",
            root.display()
        );

        let (config, _) = Config::from_yaml(&yaml, &env).expect("fixture config should load");
        config
    }
}

#[cfg(test)]
mod tests {
    use super::{testing::Harness, *};
    use crate::{
        puzzle::{Day, Year},
        report::Event,
    };
    use std::fs;

    fn puzzle() -> Puzzle {
        Puzzle::new(
            Year::new(2024).expect("valid year"),
            Day::new(7).expect("valid day"),
        )
        .expect("2024 has a day 7")
    }

    #[test]
    fn path_prints_the_project_directory_to_stdout() {
        let root = tempfile::tempdir().expect("temp dir");
        let mut harness = Harness::new(root.path());
        let project = root.path().join("2024").join("day07").join("rust");

        harness
            .app()
            .execute(Plan::Path {
                project: project.clone(),
            })
            .expect("path should succeed");

        assert_eq!(
            harness.reporter.events,
            [Event::Data(project.to_string_lossy().into_owned())]
        );
    }

    #[test]
    fn url_prints_the_puzzle_link_to_stdout() {
        let root = tempfile::tempdir().expect("temp dir");
        let mut harness = Harness::new(root.path());

        harness
            .app()
            .execute(Plan::Url { puzzle: puzzle() })
            .expect("url should succeed");

        assert_eq!(
            harness.reporter.events,
            [Event::Data(
                "https://adventofcode.com/2024/day/7".to_owned()
            )]
        );
    }

    #[test]
    fn code_launches_the_configured_editor_without_waiting() {
        let root = tempfile::tempdir().expect("temp dir");
        let project = root.path().join("2024").join("day07").join("rust");
        fs::create_dir_all(&project).expect("create project");
        let mut harness = Harness::new(root.path());

        harness
            .app()
            .execute(Plan::Code {
                project: project.clone(),
            })
            .expect("code should succeed");

        let spawned = harness.runner.spawned();
        assert_eq!(spawned.len(), 1);
        assert_eq!(spawned[0].program(), "code");
        assert_eq!(spawned[0].arguments(), [project.as_os_str()]);
        assert!(
            harness.runner.executed().is_empty(),
            "the editor must not be waited on"
        );
    }

    #[test]
    fn code_refuses_to_open_a_project_that_does_not_exist() {
        let root = tempfile::tempdir().expect("temp dir");
        let mut harness = Harness::new(root.path());

        let error = harness
            .app()
            .execute(Plan::Code {
                project: root.path().join("2024").join("day07").join("rust"),
            })
            .expect_err("project does not exist");

        assert!(matches!(error, Error::ProjectMissing { .. }), "{error:?}");
    }
}
