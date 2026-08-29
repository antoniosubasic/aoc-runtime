//! Command line surface.
//!
//! This module only describes and parses arguments. Everything that needs a
//! clock, the filesystem or configuration happens in [`crate::resolve`].

use crate::{
    language::Language,
    puzzle::{Day, Year},
    resolve::latest_available_year,
};
use chrono::NaiveDate;
use clap::{Parser, ValueEnum};
use std::{fmt, path::PathBuf};

/// Automate the Advent of Code loop: scaffold, run and submit solutions.
#[derive(Debug, Parser)]
#[command(
    name = "aoc",
    version,
    about,
    after_help = "Several modes run in the order given, stopping at the first failure.\n\n\
                  Unspecified values are recovered from the current directory using the \
                  configured path template, then fall back to today's puzzle."
)]
pub struct Cli {
    /// Puzzle year [default: the most recent event]
    #[arg(short, long, value_parser = year_parser())]
    pub year: Option<u16>,

    /// Puzzle day [default: today in December, otherwise 1]
    #[arg(short, long, value_parser = day_parser())]
    pub day: Option<u8>,

    /// Solution language
    #[arg(short, long)]
    pub language: Option<Language>,

    /// What to do with the puzzle, in the order given
    #[arg(value_enum, value_name = "MODE", default_values_t = vec![Mode::Run])]
    pub modes: Vec<Mode>,

    /// Run the solution without submitting its answers
    #[arg(long)]
    pub no_submit: bool,

    /// Use this config file instead of the default location
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
}

/// What `aoc` should do with the selected puzzle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Mode {
    /// Build the solution, run it, and submit the answers it prints.
    Run,
    /// Create the project directory and scaffold a solution in it.
    Init,
    /// Print the project directory.
    Path,
    /// Open the project directory in the configured editor.
    Code,
    /// Print the puzzle URL.
    Url,
}

impl Mode {
    /// Whether this mode needs a language to identify a project.
    #[must_use]
    pub const fn needs_language(self) -> bool {
        !matches!(self, Self::Url)
    }

    /// The mode's lowercase name, as accepted on the command line.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::Init => "init",
            Self::Path => "path",
            Self::Code => "code",
            Self::Url => "url",
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

fn year_parser() -> clap::builder::RangedI64ValueParser<u16> {
    year_parser_for(chrono::Local::now().date_naive())
}

fn year_parser_for(today: NaiveDate) -> clap::builder::RangedI64ValueParser<u16> {
    clap::value_parser!(u16)
        .range(i64::from(Year::FIRST.get())..=i64::from(latest_available_year(today).get()))
}

fn day_parser() -> clap::builder::RangedI64ValueParser<u8> {
    clap::value_parser!(u8).range(i64::from(Day::FIRST.get())..=i64::from(Day::LAST_FULL.get()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_command_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn the_mode_defaults_to_run() {
        let cli = Cli::try_parse_from(["aoc"]).expect("no arguments is valid");

        assert_eq!(cli.modes, [Mode::Run]);
        assert_eq!(cli.year, None);
        assert_eq!(cli.day, None);
        assert_eq!(cli.language, None);
        assert!(!cli.no_submit);
        assert_eq!(cli.config, None);
    }

    #[test]
    fn accepts_every_mode_positionally() {
        for mode in [Mode::Run, Mode::Init, Mode::Path, Mode::Code, Mode::Url] {
            let cli = Cli::try_parse_from(["aoc", mode.name()]).expect("mode should parse");
            assert_eq!(cli.modes, [mode]);
        }
    }

    #[test]
    fn accepts_short_and_long_flags() {
        let short = Cli::try_parse_from(["aoc", "-y", "2024", "-d", "7", "-l", "csharp", "path"])
            .expect("short flags should parse");
        let long = Cli::try_parse_from([
            "aoc",
            "--year",
            "2024",
            "--day",
            "7",
            "--language",
            "csharp",
            "path",
        ])
        .expect("long flags should parse");

        assert_eq!(short.year, Some(2024));
        assert_eq!(short.day, Some(7));
        assert_eq!(short.language, Some(Language::CSharp));
        assert_eq!(short.modes, [Mode::Path]);
        assert_eq!(long.year, short.year);
        assert_eq!(long.day, short.day);
        assert_eq!(long.language, short.language);
    }

    #[test]
    fn several_modes_parse_in_the_order_given() {
        let cli = Cli::try_parse_from(["aoc", "init", "code"]).expect("two modes should parse");

        assert_eq!(cli.modes, [Mode::Init, Mode::Code]);
    }

    #[test]
    fn flags_may_sit_between_modes() {
        let cli = Cli::try_parse_from(["aoc", "init", "-y", "2024", "path"])
            .expect("a flag between modes should parse");

        assert_eq!(cli.modes, [Mode::Init, Mode::Path]);
        assert_eq!(cli.year, Some(2024));
    }

    #[test]
    fn a_mode_may_repeat() {
        let cli = Cli::try_parse_from(["aoc", "run", "run"]).expect("a repeated mode should parse");

        assert_eq!(cli.modes, [Mode::Run, Mode::Run]);
    }

    #[test]
    fn rejects_days_outside_the_puzzle_range() {
        for day in ["0", "26", "31"] {
            assert!(
                Cli::try_parse_from(["aoc", "-d", day]).is_err(),
                "day {day}"
            );
        }
        assert!(Cli::try_parse_from(["aoc", "-d", "25"]).is_ok());
    }

    #[test]
    fn rejects_years_outside_the_available_range() {
        assert!(Cli::try_parse_from(["aoc", "-y", "2014"]).is_err());
        assert!(Cli::try_parse_from(["aoc", "-y", "2015"]).is_ok());
        assert!(
            Cli::try_parse_from(["aoc", "-y", "2999"]).is_err(),
            "a year that has not happened is a usage error"
        );
    }

    #[test]
    fn rejects_unknown_languages_and_modes() {
        assert!(Cli::try_parse_from(["aoc", "-l", "cobol"]).is_err());
        assert!(Cli::try_parse_from(["aoc", "-l", "c-sharp"]).is_err());
        assert!(Cli::try_parse_from(["aoc", "compile"]).is_err());
    }

    #[test]
    fn only_url_works_without_a_language() {
        assert!(!Mode::Url.needs_language());
        for mode in [Mode::Run, Mode::Init, Mode::Path, Mode::Code] {
            assert!(mode.needs_language(), "{mode}");
        }
    }
}
