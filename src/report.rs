//! User-facing output.
//!
//! Data goes to standard output so `cd $(aoc path)` works; diagnostics go to
//! standard error. Handlers emit semantic [`Event`]s, so tests assert on what
//! happened rather than on ANSI escape sequences.

use crate::{aoc::Verdict, puzzle::Part};
use colored::Colorize as _;
use std::io::{self, Write as _};

/// Something worth telling the user about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A machine-consumable result, such as a path or a URL.
    Data(String),
    /// A solution's own output, passed through verbatim.
    Raw(String),
    /// An answer and how the site judged it.
    Answer {
        /// Which part the answer belongs to.
        part: Part,
        /// The answer itself.
        answer: String,
        /// The site's verdict, or `None` when nothing was submitted.
        verdict: Option<Verdict>,
    },
    /// A non-fatal problem.
    Warning(String),
}

/// Receives [`Event`]s and presents them.
pub trait Reporter {
    /// Presents a single event.
    fn report(&mut self, event: Event);

    /// Emits a machine-consumable result.
    fn data(&mut self, text: &str) {
        self.report(Event::Data(text.to_owned()));
    }

    /// Emits a solution's own output.
    fn raw(&mut self, text: &str) {
        self.report(Event::Raw(text.to_owned()));
    }

    /// Emits an answer and its verdict.
    fn answer(&mut self, part: Part, answer: &str, verdict: Option<Verdict>) {
        self.report(Event::Answer {
            part,
            answer: answer.to_owned(),
            verdict,
        });
    }

    /// Emits a non-fatal problem.
    fn warn(&mut self, text: &str) {
        self.report(Event::Warning(text.to_owned()));
    }
}

/// Writes events to the terminal, colouring answers by verdict.
#[derive(Debug, Default, Clone, Copy)]
pub struct TermReporter;

impl Reporter for TermReporter {
    fn report(&mut self, event: Event) {
        match event {
            Event::Data(text) => println!("{text}"),
            Event::Raw(text) => {
                print!("{text}");
                if !text.ends_with('\n') {
                    println!();
                }
                let _ = io::stdout().flush();
            }
            Event::Answer {
                part,
                answer,
                verdict,
            } => match &verdict {
                Some(Verdict::Correct) => println!("{}", answer.green()),
                Some(Verdict::Incorrect) => println!("{}", answer.red()),
                Some(Verdict::NotAccepted { wait }) => {
                    println!("{}", answer.yellow());
                    eprintln!("{part}: not accepted - wait {wait} before trying again");
                }
                None => println!("{answer}"),
            },
            Event::Warning(text) => eprintln!("{} {text}", "warning:".yellow().bold()),
        }
    }
}

#[cfg(test)]
pub(crate) mod recording {
    use super::{Event, Reporter};

    #[derive(Debug, Default)]
    pub(crate) struct RecordingReporter {
        pub(crate) events: Vec<Event>,
    }

    impl RecordingReporter {
        pub(crate) fn new() -> Self {
            Self::default()
        }

        pub(crate) fn raw_output(&self) -> String {
            self.events
                .iter()
                .filter_map(|event| match event {
                    Event::Raw(text) => Some(text.as_str()),
                    _ => None,
                })
                .collect()
        }

        pub(crate) fn answers(&self) -> Vec<Event> {
            self.events
                .iter()
                .filter(|event| matches!(event, Event::Answer { .. }))
                .cloned()
                .collect()
        }

        pub(crate) fn warnings(&self) -> Vec<&str> {
            self.events
                .iter()
                .filter_map(|event| match event {
                    Event::Warning(text) => Some(text.as_str()),
                    _ => None,
                })
                .collect()
        }
    }

    impl Reporter for RecordingReporter {
        fn report(&mut self, event: Event) {
            self.events.push(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{recording::RecordingReporter, *};

    #[test]
    fn records_every_kind_of_event() {
        let mut reporter = RecordingReporter::new();

        reporter.data("/aoc/2024/day07/rust");
        reporter.raw("hello\n");
        reporter.answer(Part::One, "1227", Some(Verdict::Correct));
        reporter.warn("no cookie");

        assert_eq!(reporter.events.len(), 4);
        assert_eq!(reporter.raw_output(), "hello\n");
        assert_eq!(reporter.warnings(), ["no cookie"]);
        assert_eq!(
            reporter.events[0],
            Event::Data("/aoc/2024/day07/rust".to_owned())
        );
        assert_eq!(
            reporter.events[2],
            Event::Answer {
                part: Part::One,
                answer: "1227".to_owned(),
                verdict: Some(Verdict::Correct),
            }
        );
    }
}
