//! `aoc run`: build the solution, execute it, and submit what it prints.

use super::App;
use crate::{
    answer::{Answers, Outcome, classify},
    aoc::{AocClient, Verdict},
    error::Error,
    language::{Language, LanguageCommands},
    process::{CommandOutput, IoPolicy, ProcessError},
    puzzle::Puzzle,
};
use std::path::Path;

impl App<'_> {
    pub(super) fn run(
        &mut self,
        puzzle: Puzzle,
        language: Language,
        project: &Path,
        submit: bool,
    ) -> Result<(), Error> {
        if !project.exists() {
            return Err(Error::ProjectMissing {
                path: project.to_path_buf(),
            });
        }

        self.ensure_input(puzzle, project);

        let commands = language.commands(project)?;
        if let Some(build) = &commands.build {
            self.runner.run_checked(build, IoPolicy::SILENT)?;
        }

        let output = self.execute_solution(&commands)?;

        match (classify(&output.stdout), submit, self.client) {
            (Outcome::Answers(answers), true, Some(client)) => {
                self.submit_answers(client, puzzle, &answers)
            }
            (Outcome::Answers(answers), _, _) => {
                for (part, answer) in answers.iter() {
                    self.reporter.answer(part, answer, None);
                }
                Ok(())
            }
            (Outcome::Raw, _, _) => {
                self.reporter.raw(&output.stdout);
                Ok(())
            }
        }
    }

    fn execute_solution(&self, commands: &LanguageCommands) -> Result<CommandOutput, Error> {
        match self.runner.run_checked(&commands.run, IoPolicy::ANSWER) {
            Err(ProcessError::NotFound { program }) => match &commands.run_fallback {
                Some(fallback) => Ok(self.runner.run_checked(fallback, IoPolicy::ANSWER)?),
                None => Err(ProcessError::NotFound { program }.into()),
            },
            other => Ok(other?),
        }
    }

    fn submit_answers(
        &mut self,
        client: &dyn AocClient,
        puzzle: Puzzle,
        answers: &Answers,
    ) -> Result<(), Error> {
        for (part, answer) in answers.iter() {
            let verdict = match self.cache.correct(puzzle, part) {
                Some(known) if known == answer => Verdict::Correct,
                Some(_) => Verdict::Incorrect {
                    hint: None,
                    wait: None,
                },
                None => {
                    let verdict = client.submit(puzzle, part, answer)?;
                    if verdict == Verdict::Correct {
                        self.cache.record(puzzle, part, answer);
                    }
                    verdict
                }
            };

            // A wait means nothing more can be judged for now, so submitting
            // the next part would only spend a request on being refused.
            let waiting = verdict.wait().is_some();
            self.reporter.answer(part, answer, Some(verdict));

            if waiting {
                break;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aoc::{
            cache::{AnswerCache as _, memory::MemoryCache},
            fake::FakeClient,
        },
        app::{INPUT_FILE_NAME, testing::Harness},
        puzzle::{Day, Part, Year},
        report::Event,
    };
    use std::{fs, time::Duration};
    use tempfile::TempDir;

    /// A plain rejection, with nothing the site chose to add to it.
    fn wrong() -> Verdict {
        Verdict::Incorrect {
            hint: None,
            wait: None,
        }
    }

    fn puzzle() -> Puzzle {
        Puzzle::new(
            Year::new(2024).expect("valid year"),
            Day::new(7).expect("valid day"),
        )
        .expect("2024 has a day 7")
    }

    fn scaffold() -> (TempDir, std::path::PathBuf) {
        let root = tempfile::tempdir().expect("temp dir");
        let project = root.path().join("2024").join("day07").join("rust");
        fs::create_dir_all(&project).expect("create project");
        (root, project)
    }

    fn programs(harness: &Harness) -> Vec<String> {
        harness
            .runner
            .executed()
            .iter()
            .map(|spec| spec.program().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn builds_before_running() {
        let (root, project) = scaffold();
        let mut harness = Harness::new(root.path());
        harness.runner.push_stdout("").push_stdout("1227\n");

        harness
            .app()
            .run(puzzle(), Language::Rust, &project, false)
            .expect("run should succeed");

        assert_eq!(programs(&harness), ["cargo", "cargo"]);
        assert_eq!(
            harness.runner.policies(),
            [IoPolicy::SILENT, IoPolicy::ANSWER]
        );
    }

    #[test]
    fn skips_the_build_for_interpreted_languages() {
        let root = tempfile::tempdir().expect("temp dir");
        let project = root.path().join("2024").join("day07").join("python");
        fs::create_dir_all(&project).expect("create project");
        let mut harness = Harness::new(root.path());
        harness.runner.push_stdout("1227\n");

        harness
            .app()
            .run(puzzle(), Language::Python, &project, false)
            .expect("run should succeed");

        assert_eq!(programs(&harness), ["python3"]);
    }

    #[test]
    fn a_failing_build_stops_before_the_solution_runs() {
        let (root, project) = scaffold();
        let mut harness = Harness::new(root.path());
        harness.runner.push_failure(101, "error: could not compile");

        let error = harness
            .app()
            .run(puzzle(), Language::Rust, &project, false)
            .expect_err("build failed");

        assert!(error.to_string().contains("could not compile"), "{error}");
        assert_eq!(harness.runner.executed().len(), 1);
    }

    #[test]
    fn a_missing_project_is_an_error() {
        let root = tempfile::tempdir().expect("temp dir");
        let mut harness = Harness::new(root.path());
        let project = root.path().join("2024").join("day07").join("rust");

        let error = harness
            .app()
            .run(puzzle(), Language::Rust, &project, false)
            .expect_err("project does not exist");

        assert!(matches!(error, Error::ProjectMissing { .. }), "{error:?}");
        assert!(harness.runner.executed().is_empty());
    }

    #[test]
    fn unstructured_output_is_passed_through_untouched() {
        let (root, project) = scaffold();
        let mut harness = Harness::new(root.path()).with_client(FakeClient::new());
        harness.runner.push_stdout("").push_stdout("a\nb\nc\n");

        harness
            .app()
            .run(puzzle(), Language::Rust, &project, true)
            .expect("run should succeed");

        assert_eq!(harness.reporter.raw_output(), "a\nb\nc\n");
        assert!(
            harness
                .client
                .as_ref()
                .expect("client")
                .submitted()
                .is_empty()
        );
    }

    #[test]
    fn submits_both_parts_and_reports_verdicts() {
        let (root, project) = scaffold();
        let client = FakeClient::new();
        client.push(Verdict::Correct).push(wrong());
        let mut harness = Harness::new(root.path()).with_client(client);
        harness.runner.push_stdout("").push_stdout("1227\n23262\n");

        harness
            .app()
            .run(puzzle(), Language::Rust, &project, true)
            .expect("run should succeed");

        let submitted = harness.client.as_ref().expect("client").submitted();
        assert_eq!(submitted.len(), 2);
        assert_eq!(submitted[0], (puzzle(), Part::One, "1227".to_owned()));
        assert_eq!(submitted[1], (puzzle(), Part::Two, "23262".to_owned()));

        assert_eq!(
            harness.reporter.answers(),
            [
                Event::Answer {
                    part: Part::One,
                    answer: "1227".to_owned(),
                    verdict: Some(Verdict::Correct),
                },
                Event::Answer {
                    part: Part::Two,
                    answer: "23262".to_owned(),
                    verdict: Some(wrong()),
                },
            ]
        );
    }

    #[test]
    fn never_submits_without_a_client() {
        let (root, project) = scaffold();
        let mut harness = Harness::new(root.path());
        harness.runner.push_stdout("").push_stdout("1227\n");

        harness
            .app()
            .run(puzzle(), Language::Rust, &project, true)
            .expect("run should succeed");

        assert_eq!(
            harness.reporter.answers(),
            [Event::Answer {
                part: Part::One,
                answer: "1227".to_owned(),
                verdict: None,
            }]
        );
    }

    #[test]
    fn no_submit_runs_the_solution_without_submitting() {
        let (root, project) = scaffold();
        let mut harness = Harness::new(root.path()).with_client(FakeClient::new());
        harness.runner.push_stdout("").push_stdout("1227\n");

        harness
            .app()
            .run(puzzle(), Language::Rust, &project, false)
            .expect("run should succeed");

        assert!(
            harness
                .client
                .as_ref()
                .expect("client")
                .submitted()
                .is_empty()
        );
    }

    #[test]
    fn a_cached_answer_is_verified_without_a_request() {
        let (root, project) = scaffold();
        let mut harness = Harness::new(root.path())
            .with_client(FakeClient::new())
            .with_cache(MemoryCache::seeded(puzzle(), Part::One, "1227"));
        harness.runner.push_stdout("").push_stdout("1227\n");

        harness
            .app()
            .run(puzzle(), Language::Rust, &project, true)
            .expect("run should succeed");

        assert!(
            harness
                .client
                .as_ref()
                .expect("client")
                .submitted()
                .is_empty(),
            "an already accepted answer must not be re-submitted"
        );
        assert_eq!(
            harness.reporter.answers(),
            [Event::Answer {
                part: Part::One,
                answer: "1227".to_owned(),
                verdict: Some(Verdict::Correct),
            }]
        );
    }

    #[test]
    fn a_correct_answer_is_remembered() {
        let (root, project) = scaffold();
        let client = FakeClient::new();
        client.push(Verdict::Correct);
        let mut harness = Harness::new(root.path()).with_client(client);
        harness.runner.push_stdout("").push_stdout("1227\n");

        harness
            .app()
            .run(puzzle(), Language::Rust, &project, true)
            .expect("run should succeed");

        assert_eq!(
            harness.cache.correct(puzzle(), Part::One).as_deref(),
            Some("1227")
        );
    }

    #[test]
    fn a_cooldown_stops_before_the_second_part() {
        let (root, project) = scaffold();
        let client = FakeClient::new();
        client.push(Verdict::Cooldown {
            wait: Duration::from_secs(200),
        });
        let mut harness = Harness::new(root.path()).with_client(client);
        harness.runner.push_stdout("").push_stdout("1227\n23262\n");

        harness
            .app()
            .run(puzzle(), Language::Rust, &project, true)
            .expect("a cooldown is not a failure");

        assert_eq!(
            harness.client.as_ref().expect("client").submitted().len(),
            1
        );
        assert_eq!(harness.reporter.answers().len(), 1);
    }

    // A rejected answer comes back with the wait the site attached to it, so
    // the next part is known to be refused before it is ever sent.
    #[test]
    fn a_wrong_answer_the_site_asks_you_to_wait_after_stops_too() {
        let (root, project) = scaffold();
        let client = FakeClient::new();
        client.push(Verdict::Incorrect {
            hint: Some(crate::aoc::Hint::TooHigh),
            wait: Some(Duration::from_secs(60)),
        });
        let mut harness = Harness::new(root.path()).with_client(client);
        harness.runner.push_stdout("").push_stdout("1227\n23262\n");

        harness
            .app()
            .run(puzzle(), Language::Rust, &project, true)
            .expect("a rejected answer is not a failure");

        assert_eq!(
            harness.client.as_ref().expect("client").submitted().len(),
            1
        );
        assert_eq!(harness.reporter.answers().len(), 1);
    }

    #[test]
    fn downloads_the_input_beside_the_project_when_missing() {
        let (root, project) = scaffold();
        let mut harness =
            Harness::new(root.path()).with_client(FakeClient::with_input("puzzle input"));
        harness.runner.push_stdout("").push_stdout("1227\n");

        harness
            .app()
            .run(puzzle(), Language::Rust, &project, false)
            .expect("run should succeed");

        let input = root.path().join("2024").join("day07").join(INPUT_FILE_NAME);
        assert_eq!(
            fs::read_to_string(input).expect("input should exist"),
            "puzzle input"
        );
        assert!(
            harness.inputs.holds(puzzle()),
            "the download must be kept where a later project can reuse it"
        );
    }

    #[test]
    fn a_cached_input_is_linked_rather_than_downloaded_again() {
        let (root, project) = scaffold();
        let mut harness =
            Harness::new(root.path()).with_client(FakeClient::with_input("downloaded"));
        harness
            .inputs
            .store(puzzle(), "cached")
            .expect("seed the input cache");
        harness.runner.push_stdout("").push_stdout("1227\n");

        harness
            .app()
            .run(puzzle(), Language::Rust, &project, false)
            .expect("run should succeed");

        let input = root.path().join("2024").join("day07").join(INPUT_FILE_NAME);
        assert_eq!(
            fs::read_to_string(input).expect("input should exist"),
            "cached",
            "a puzzle already downloaded must not be fetched a second time"
        );
    }

    // Deleting `input.txt` used to be how you forced a fresh download. It now
    // costs a link, because the copy that matters never left the state
    // directory.
    #[test]
    fn a_project_that_lost_its_input_is_relinked_without_a_request() {
        let (root, project) = scaffold();
        let mut harness =
            Harness::new(root.path()).with_client(FakeClient::with_input("downloaded"));
        harness
            .runner
            .push_stdout("")
            .push_stdout("1227\n")
            .push_stdout("")
            .push_stdout("1227\n");

        harness
            .app()
            .run(puzzle(), Language::Rust, &project, false)
            .expect("first run downloads the input");

        // Marking the cached copy makes it obvious which one is being read.
        fs::write(harness.inputs.path(puzzle()), "cached").expect("mark the cached input");
        let input = root.path().join("2024").join("day07").join(INPUT_FILE_NAME);
        fs::remove_file(&input).expect("delete the project's input");

        harness
            .app()
            .run(puzzle(), Language::Rust, &project, false)
            .expect("second run relinks the input");

        assert_eq!(
            fs::read_to_string(&input).expect("input should exist"),
            "cached"
        );
    }

    #[test]
    fn an_existing_input_is_left_alone() {
        let (root, project) = scaffold();
        let input = root.path().join("2024").join("day07").join(INPUT_FILE_NAME);
        fs::write(&input, "original").expect("seed input");

        let mut harness =
            Harness::new(root.path()).with_client(FakeClient::with_input("downloaded"));
        harness.runner.push_stdout("").push_stdout("1227\n");

        harness
            .app()
            .run(puzzle(), Language::Rust, &project, false)
            .expect("run should succeed");

        assert_eq!(fs::read_to_string(input).expect("input"), "original");
    }

    #[test]
    fn a_failed_download_warns_instead_of_aborting() {
        let (root, project) = scaffold();
        let mut harness = Harness::new(root.path()).with_client(FakeClient::new());
        harness.runner.push_stdout("").push_stdout("1227\n");

        harness
            .app()
            .run(puzzle(), Language::Rust, &project, false)
            .expect("a failed download must not stop the run");

        assert!(
            harness
                .reporter
                .warnings()
                .iter()
                .any(|warning| warning.contains("could not download")),
            "{:?}",
            harness.reporter.warnings()
        );
    }

    #[test]
    fn a_missing_cookie_warns_that_no_input_was_fetched() {
        let (root, project) = scaffold();
        let mut harness = Harness::new(root.path());
        harness.runner.push_stdout("").push_stdout("1227\n");

        harness
            .app()
            .run(puzzle(), Language::Rust, &project, false)
            .expect("run should succeed");

        assert!(
            harness
                .reporter
                .warnings()
                .iter()
                .any(|warning| warning.contains("no session cookie")),
            "{:?}",
            harness.reporter.warnings()
        );
    }
}
