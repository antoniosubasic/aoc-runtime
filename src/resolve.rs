//! Turning command line arguments into a concrete plan of work.
//!
//! Resolution is pure: it takes the parsed arguments, the configuration, a
//! directory and a clock, and produces one [`Plan`] per requested mode, whose
//! variants carry exactly what their handler needs. A mode that requires a
//! language cannot be constructed without one, so no downstream code has to
//! re-check.

use crate::{
    cli::{Cli, Mode},
    config::Config,
    env::{Clock, is_december},
    language::Language,
    puzzle::{Day, Puzzle, Year},
    template::{Params, TemplateError},
};
use chrono::{Datelike, NaiveDate};
use std::path::{Path, PathBuf};

/// A fully resolved unit of work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Build and run a solution, optionally submitting its answers.
    Run {
        /// The puzzle being solved.
        puzzle: Puzzle,
        /// The language the solution is written in.
        language: Language,
        /// The project directory.
        project: PathBuf,
        /// Whether answers should be submitted.
        submit: bool,
    },
    /// Scaffold a new solution.
    Init {
        /// The puzzle being solved.
        puzzle: Puzzle,
        /// The language to scaffold.
        language: Language,
        /// The project directory.
        project: PathBuf,
    },
    /// Print a project directory.
    Path {
        /// The project directory.
        project: PathBuf,
    },
    /// Open a project directory in the editor.
    Code {
        /// The project directory.
        project: PathBuf,
    },
    /// Print a puzzle URL.
    Url {
        /// The puzzle to link to.
        puzzle: Puzzle,
    },
    /// Open a puzzle URL in the default browser.
    Open {
        /// The puzzle to open.
        puzzle: Puzzle,
    },
    /// Empty the state directory.
    Clean {
        /// Whether cached inputs and answers go too, rather than build output
        /// alone.
        all: bool,
    },
}

/// Resolves arguments, the working directory and the clock into one [`Plan`]
/// per requested mode, in the order the modes were given.
///
/// Precedence for each value is: explicit argument, then whatever the working
/// directory reveals through the configured template, then a date-based
/// default. The puzzle and the project directory are shared by every mode that
/// names one; no mode observes what the ones before it did. `clean` names
/// neither: it works on the state directory alone, so it is planned without a
/// puzzle, a language or a project, and the flags that belong to it are
/// refused anywhere else.
///
/// # Errors
///
/// Returns [`ResolveError::CleanOnlyFlag`] if `--all` or `--yes` was given
/// without `clean`, [`ResolveError::LanguageRequired`] if a mode needs a
/// language and none could be determined, [`ResolveError::DayOutOfRange`] if a
/// mode names a puzzle the event never had, or [`ResolveError::Template`] if
/// the template's matcher cannot be compiled. Either way nothing is returned,
/// so a failing mode is caught before any of them executes.
pub fn plan(
    cli: &Cli,
    config: &Config,
    cwd: &Path,
    clock: &dyn Clock,
) -> Result<Vec<Plan>, ResolveError> {
    // `--all` and `--yes` are `clean`'s alone. Anywhere else they would quietly
    // do nothing, and `aoc run --all` reads like a promise to run every day.
    if !cli.modes.contains(&Mode::Clean) {
        if cli.all {
            return Err(ResolveError::CleanOnlyFlag { flag: "all" });
        }
        if cli.yes {
            return Err(ResolveError::CleanOnlyFlag { flag: "yes" });
        }
    }

    let today = clock.today();
    let detected = config.template.matcher()?.detect(cwd);

    let year = cli
        .year
        .and_then(Year::new)
        .or(detected.year)
        .unwrap_or_else(|| latest_available_year(today));

    let day = cli
        .day
        .and_then(Day::new)
        .or_else(|| detected.day.filter(|&day| year.has_day(day)))
        .unwrap_or_else(|| default_day(year, today));

    let language = cli.language.or(detected.language);
    // Only the modes that name a puzzle care whether this pairing exists, so
    // the pairing is checked where it is used rather than for every mode.
    let puzzle = || Puzzle::new(year, day).ok_or(ResolveError::DayOutOfRange { year, day });

    cli.modes
        .iter()
        .map(|&mode| {
            let with_project = || -> Result<Project, ResolveError> {
                let language = language.ok_or(ResolveError::LanguageRequired { mode })?;

                Ok(Project {
                    puzzle: puzzle()?,
                    language,
                    path: config.template.render(Params {
                        year,
                        day,
                        language,
                    }),
                })
            };

            Ok(match mode {
                Mode::Clean => Plan::Clean { all: cli.all },
                Mode::Url => Plan::Url { puzzle: puzzle()? },
                Mode::Open => Plan::Open { puzzle: puzzle()? },
                Mode::Run => {
                    let project = with_project()?;
                    Plan::Run {
                        puzzle: project.puzzle,
                        language: project.language,
                        project: project.path,
                        submit: !cli.no_submit,
                    }
                }
                Mode::Init => {
                    let project = with_project()?;
                    Plan::Init {
                        puzzle: project.puzzle,
                        language: project.language,
                        project: project.path,
                    }
                }
                Mode::Path => Plan::Path {
                    project: with_project()?.path,
                },
                Mode::Code => Plan::Code {
                    project: with_project()?.path,
                },
            })
        })
        .collect()
}

/// A project directory together with the puzzle and language it was rendered
/// from, which the modes that work on a project need in various combinations.
struct Project {
    puzzle: Puzzle,
    language: Language,
    path: PathBuf,
}

/// The most recent event that has started on the given date.
///
/// Advent of Code begins on 1 December, so before December the current year's
/// event does not exist yet.
#[must_use]
pub fn latest_available_year(today: NaiveDate) -> Year {
    let year = today.year();
    let available = if is_december(today) { year } else { year - 1 };

    u16::try_from(available)
        .ok()
        .and_then(Year::new)
        .unwrap_or(Year::FIRST)
}

/// The day to use when none was given: today during the event, otherwise the
/// first puzzle.
///
/// Today is clamped to the end of `year`'s event, which is day 12 from
/// [`Year::FIRST_SHORT`] on.
#[must_use]
pub fn default_day(year: Year, today: NaiveDate) -> Day {
    if is_december(today) {
        Day::clamped(year, today.day())
    } else {
        Day::FIRST
    }
}

/// Errors produced while resolving a plan.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    /// The mode needs a language and none was given or detected.
    #[error(
        "a language is required for `{mode}` - pass --language, or run from a \
         directory the template can resolve"
    )]
    LanguageRequired {
        /// The mode that needs a language.
        mode: Mode,
    },
    /// A flag only `clean` understands was given without it.
    #[error("`--{flag}` applies to `clean` only, which is not among the modes given")]
    CleanOnlyFlag {
        /// The flag's long name, without its dashes.
        flag: &'static str,
    },
    /// The requested day is past the end of the requested event.
    #[error("{year} has no day {day} - that event ends on day {}", .year.last_day())]
    DayOutOfRange {
        /// The event the day was requested in.
        year: Year,
        /// The requested day.
        day: Day,
    },
    /// The template could not be compiled into a matcher.
    #[error("could not build a matcher from the configured template")]
    Template(#[from] TemplateError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::{Env, FixedClock};
    use clap::Parser as _;

    /// The drive the paths in these tests sit on. A path that merely starts
    /// with a separator is drive-relative on Windows, and a template that is
    /// not absolute is refused.
    const DRIVE: &str = if cfg!(windows) { "C:" } else { "" };

    /// The template every plan here is resolved against, below `DRIVE`.
    const TEMPLATE: &str = "/root/{{year}}/day{{pad day}}/{{language}}";

    /// A path below `DRIVE`, spelled the way these tests spell one.
    fn rooted(path: &str) -> PathBuf {
        PathBuf::from(format!("{DRIVE}{path}"))
    }

    fn env(cwd: &str) -> Env {
        Env {
            home: rooted("/home/tester"),
            config_dir: rooted("/home/tester/.config/aoc"),
            config_file: rooted("/home/tester/.config/aoc/config.yaml"),
            state_dir: rooted("/home/tester/.local/state/aoc"),
            cwd: rooted(cwd),
            session_cookie: None,
        }
    }

    fn config() -> Config {
        let (config, _) =
            Config::from_yaml(&format!("template_path: \"{DRIVE}{TEMPLATE}\""), &env("/"))
                .expect("config should load");
        config
    }

    fn cli(args: &[&str]) -> Cli {
        let mut command_line = vec!["aoc"];
        command_line.extend_from_slice(args);
        Cli::try_parse_from(command_line).expect("arguments should parse")
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn year(year: u16) -> Year {
        Year::new(year).expect("valid year")
    }

    fn resolve_all(args: &[&str], cwd: &str, today: NaiveDate) -> Result<Vec<Plan>, ResolveError> {
        plan(&cli(args), &config(), &rooted(cwd), &FixedClock(today))
    }

    fn resolve_one(args: &[&str], cwd: &str, today: NaiveDate) -> Result<Plan, ResolveError> {
        resolve_all(args, cwd, today).map(|mut plans| {
            assert_eq!(plans.len(), 1, "a single mode should produce a single plan");
            plans.pop().expect("just asserted there is one")
        })
    }

    fn puzzle_of(plan: &Plan) -> Option<Puzzle> {
        match plan {
            Plan::Run { puzzle, .. }
            | Plan::Init { puzzle, .. }
            | Plan::Url { puzzle }
            | Plan::Open { puzzle } => Some(*puzzle),
            Plan::Path { .. } | Plan::Code { .. } | Plan::Clean { .. } => None,
        }
    }

    fn project_of(plan: &Plan) -> Option<&Path> {
        match plan {
            Plan::Run { project, .. }
            | Plan::Init { project, .. }
            | Plan::Path { project }
            | Plan::Code { project } => Some(project),
            Plan::Url { .. } | Plan::Open { .. } | Plan::Clean { .. } => None,
        }
    }

    #[test]
    fn the_most_recent_event_depends_on_the_month() {
        assert_eq!(latest_available_year(date(2024, 12, 1)).get(), 2024);
        assert_eq!(latest_available_year(date(2024, 12, 31)).get(), 2024);
        assert_eq!(latest_available_year(date(2024, 11, 30)).get(), 2023);
        assert_eq!(latest_available_year(date(2025, 1, 1)).get(), 2024);
    }

    #[test]
    fn the_default_day_is_today_during_the_event() {
        assert_eq!(default_day(year(2024), date(2024, 12, 1)).get(), 1);
        assert_eq!(default_day(year(2024), date(2024, 12, 14)).get(), 14);
        assert_eq!(default_day(year(2024), date(2024, 12, 25)).get(), 25);
    }

    #[test]
    fn the_default_day_is_clamped_after_the_event_ends() {
        for day in 26..=31 {
            assert_eq!(
                default_day(year(2024), date(2024, 12, day)).get(),
                25,
                "december {day}"
            );
        }
    }

    #[test]
    fn the_default_day_is_clamped_to_a_shortened_event() {
        for day in 13..=31 {
            assert_eq!(
                default_day(year(2025), date(2025, 12, day)).get(),
                12,
                "december {day}"
            );
        }
    }

    #[test]
    fn the_default_day_is_the_first_outside_december() {
        assert_eq!(default_day(year(2024), date(2024, 1, 20)).get(), 1);
        assert_eq!(default_day(year(2024), date(2024, 11, 30)).get(), 1);
    }

    #[test]
    fn explicit_arguments_win_over_everything() {
        let plan = resolve_one(
            &["-y", "2019", "-d", "3", "-l", "java", "path"],
            "/root/2024/day07/rust",
            date(2024, 12, 14),
        )
        .expect("plan should resolve");

        assert_eq!(
            project_of(&plan),
            Some(rooted("/root/2019/day03/java").as_path())
        );
    }

    #[test]
    fn the_working_directory_wins_over_date_defaults() {
        let plan = resolve_one(&["path"], "/root/2019/day03/java", date(2024, 12, 14))
            .expect("plan should resolve");

        assert_eq!(
            project_of(&plan),
            Some(rooted("/root/2019/day03/java").as_path())
        );
    }

    #[test]
    fn date_defaults_apply_when_nothing_else_does() {
        let plan = resolve_one(&["-l", "rust", "path"], "/elsewhere", date(2024, 12, 14))
            .expect("plan should resolve");

        assert_eq!(
            project_of(&plan),
            Some(rooted("/root/2024/day14/rust").as_path())
        );
    }

    #[test]
    fn a_partial_directory_fills_the_rest_from_defaults() {
        let plan = resolve_one(&["-l", "rust", "path"], "/root/2019", date(2024, 12, 14))
            .expect("plan should resolve");

        assert_eq!(
            project_of(&plan),
            Some(rooted("/root/2019/day14/rust").as_path())
        );
    }

    #[test]
    fn an_explicit_day_the_event_never_had_is_an_error() {
        let error = resolve_one(
            &["-y", "2025", "-d", "20", "url"],
            "/elsewhere",
            date(2025, 12, 20),
        )
        .expect_err("2025 ends on day 12");

        assert!(
            matches!(error, ResolveError::DayOutOfRange { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("2025 has no day 20"), "{error}");
    }

    #[test]
    fn a_detected_day_the_event_never_had_falls_back_to_the_default() {
        let plan = resolve_one(&["path"], "/root/2025/day20/rust", date(2025, 12, 5))
            .expect("plan should resolve");

        assert_eq!(
            project_of(&plan),
            Some(rooted("/root/2025/day05/rust").as_path())
        );
    }

    #[test]
    fn a_shortened_event_still_accepts_its_own_days() {
        let plan = resolve_one(
            &["-y", "2025", "-d", "12", "url"],
            "/elsewhere",
            date(2026, 1, 1),
        )
        .expect("plan should resolve");

        assert_eq!(
            puzzle_of(&plan).map(Puzzle::url).as_deref(),
            Some("https://adventofcode.com/2025/day/12")
        );
    }

    #[test]
    fn url_needs_no_language() {
        let plan =
            resolve_one(&["url"], "/elsewhere", date(2024, 12, 5)).expect("plan should resolve");

        assert_eq!(
            plan,
            Plan::Url {
                puzzle: puzzle_of(&plan).expect("url carries a puzzle")
            }
        );
        assert_eq!(
            puzzle_of(&plan).map(Puzzle::url).as_deref(),
            Some("https://adventofcode.com/2024/day/5")
        );
    }

    #[test]
    fn open_needs_no_language() {
        let plan =
            resolve_one(&["open"], "/elsewhere", date(2024, 12, 5)).expect("plan should resolve");

        assert_eq!(
            plan,
            Plan::Open {
                puzzle: puzzle_of(&plan).expect("open carries a puzzle")
            }
        );
        assert_eq!(
            puzzle_of(&plan).map(Puzzle::url).as_deref(),
            Some("https://adventofcode.com/2024/day/5")
        );
    }

    #[test]
    fn clean_needs_no_language() {
        let plan =
            resolve_one(&["clean"], "/elsewhere", date(2024, 12, 5)).expect("plan should resolve");

        assert_eq!(plan, Plan::Clean { all: false });
    }

    #[test]
    fn clean_carries_the_all_flag() {
        let plan = resolve_one(&["clean", "--all"], "/elsewhere", date(2024, 12, 5))
            .expect("plan should resolve");

        assert_eq!(plan, Plan::Clean { all: true });
    }

    #[test]
    fn clean_is_planned_without_a_puzzle_at_all() {
        // 2025 ends on day 12, which `clean` has no opinion about: it empties
        // the state directory and names no puzzle.
        let plan = resolve_one(
            &["clean", "--all", "-y", "2025", "-d", "20"],
            "/elsewhere",
            date(2025, 12, 20),
        )
        .expect("clean needs no puzzle");

        assert_eq!(plan, Plan::Clean { all: true });
    }

    #[test]
    fn a_project_mode_still_refuses_a_day_the_event_never_had() {
        let error = resolve_one(
            &["-l", "rust", "-y", "2025", "-d", "20", "path"],
            "/elsewhere",
            date(2025, 12, 20),
        )
        .expect_err("2025 ends on day 12");

        assert!(
            matches!(error, ResolveError::DayOutOfRange { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn cleans_own_flags_are_refused_on_every_other_mode() {
        for mode in ["run", "init", "path", "code", "url", "open"] {
            for (argument, name) in [("--all", "all"), ("--yes", "yes")] {
                let error = resolve_one(
                    &["-l", "rust", argument, mode],
                    "/elsewhere",
                    date(2024, 12, 5),
                )
                .expect_err("the flag belongs to clean");

                assert!(
                    matches!(&error, ResolveError::CleanOnlyFlag { flag } if *flag == name),
                    "{mode} {argument}: {error:?}"
                );
                assert!(error.to_string().contains("clean"), "{error}");
            }
        }
    }

    #[test]
    fn cleans_own_flags_are_accepted_when_it_is_among_the_modes() {
        let plans = resolve_all(
            &["-l", "rust", "path", "clean", "--all", "--yes"],
            "/elsewhere",
            date(2024, 12, 5),
        )
        .expect("clean is among the modes");

        assert_eq!(plans.last(), Some(&Plan::Clean { all: true }));
    }

    #[test]
    fn other_modes_require_a_language() {
        for mode in ["run", "init", "path", "code"] {
            let error = resolve_one(&[mode], "/elsewhere", date(2024, 12, 5))
                .expect_err("language is required");

            assert!(
                matches!(error, ResolveError::LanguageRequired { .. }),
                "{mode}: {error:?}"
            );
            assert!(error.to_string().contains(mode), "{error}");
        }
    }

    #[test]
    fn run_submits_unless_told_otherwise() {
        let submitting = resolve_one(
            &["-l", "rust", "run"],
            "/root/2024/day07/rust",
            date(2024, 12, 7),
        )
        .expect("plan should resolve");
        let quiet = resolve_one(
            &["-l", "rust", "--no-submit", "run"],
            "/root/2024/day07/rust",
            date(2024, 12, 7),
        )
        .expect("plan should resolve");

        assert!(matches!(submitting, Plan::Run { submit: true, .. }));
        assert!(matches!(quiet, Plan::Run { submit: false, .. }));
    }

    #[test]
    fn every_mode_gets_a_plan_in_the_order_given() {
        let plans = resolve_all(
            &["init", "code", "url", "open", "clean"],
            "/root/2024/day07/rust",
            date(2024, 12, 7),
        )
        .expect("plans should resolve");

        assert!(
            matches!(
                plans.as_slice(),
                [
                    Plan::Init { .. },
                    Plan::Code { .. },
                    Plan::Url { .. },
                    Plan::Open { .. },
                    Plan::Clean { .. }
                ]
            ),
            "{plans:?}"
        );
    }

    #[test]
    fn a_missing_language_fails_before_any_plan_is_made() {
        let error = resolve_all(&["url", "path"], "/elsewhere", date(2024, 12, 5))
            .expect_err("path needs a language");

        assert!(
            matches!(error, ResolveError::LanguageRequired { mode: Mode::Path }),
            "{error:?}"
        );
    }

    #[test]
    fn each_mode_produces_its_own_plan() {
        let cwd = "/root/2024/day07/rust";
        let today = date(2024, 12, 7);

        assert!(matches!(
            resolve_one(&["run"], cwd, today),
            Ok(Plan::Run { .. })
        ));
        assert!(matches!(
            resolve_one(&["init"], cwd, today),
            Ok(Plan::Init { .. })
        ));
        assert!(matches!(
            resolve_one(&["path"], cwd, today),
            Ok(Plan::Path { .. })
        ));
        assert!(matches!(
            resolve_one(&["code"], cwd, today),
            Ok(Plan::Code { .. })
        ));
        assert!(matches!(
            resolve_one(&["url"], cwd, today),
            Ok(Plan::Url { .. })
        ));
        assert!(matches!(
            resolve_one(&["open"], cwd, today),
            Ok(Plan::Open { .. })
        ));
        assert!(matches!(
            resolve_one(&["clean"], cwd, today),
            Ok(Plan::Clean { .. })
        ));
    }
}
