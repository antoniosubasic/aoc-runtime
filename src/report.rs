//! User-facing output, and the one question the tool asks back.
//!
//! Data goes to standard output so `cd $(aoc path)` works; diagnostics go to
//! standard error. Handlers emit semantic [`Event`]s, so tests assert on what
//! happened rather than on ANSI escape sequences. A confirmation is not an
//! event - it needs an answer - so [`Confirm`] is its own seam.

use crate::{aoc::Verdict, error::Error, puzzle::Part};
use colored::Colorize as _;
use std::io::{self, BufRead as _, IsTerminal as _, Write as _};

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
                None => println!("{answer}"),
                Some(Verdict::Correct) => println!("{}", answer.green()),
                Some(verdict) => {
                    // Red is an answer the site judged and rejected; yellow is
                    // one it declined to judge, which says nothing about the
                    // answer itself.
                    let coloured = if verdict.is_judged() {
                        answer.red()
                    } else {
                        answer.yellow()
                    };

                    println!("{coloured}");
                    eprintln!("{part}: {verdict}");
                }
            },
            Event::Warning(text) => eprintln!("{} {text}", "warning:".yellow().bold()),
        }
    }
}

/// Asks the user to approve something irreversible.
pub trait Confirm {
    /// Asks `question` and reports whether the user approved.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ConfirmationRequired`] if there is nobody to ask.
    fn confirm(&mut self, question: &str) -> Result<bool, Error>;
}

/// Asks on the terminal, unless the answer was already given on the command
/// line.
///
/// Refusing when there is no terminal to ask on is the point: a destructive
/// command that cannot ask must not assume the answer it wants. Both streams
/// it uses have to be a terminal - the question goes to standard error and the
/// answer comes back on standard input - or the user would be left waiting on
/// a prompt that was written somewhere they cannot see.
#[derive(Debug, Default, Clone, Copy)]
pub struct TermConfirm {
    assume_yes: bool,
}

impl TermConfirm {
    /// Creates a prompt, answering itself when `assume_yes` is set.
    #[must_use]
    pub const fn new(assume_yes: bool) -> Self {
        Self { assume_yes }
    }
}

impl Confirm for TermConfirm {
    fn confirm(&mut self, question: &str) -> Result<bool, Error> {
        if self.assume_yes {
            return Ok(true);
        }

        let stdin = io::stdin();
        if !stdin.is_terminal() || !io::stderr().is_terminal() {
            return Err(Error::ConfirmationRequired);
        }

        // The question shares standard error with the warnings, leaving stdout
        // to the data a caller might be capturing.
        eprint!("{question} [y/N] ");
        let _ = io::stderr().flush();

        let mut answer = String::new();
        if stdin.lock().read_line(&mut answer).is_err() {
            return Ok(false);
        }

        Ok(matches!(
            answer.trim().to_ascii_lowercase().as_str(),
            "y" | "yes"
        ))
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
pub(crate) mod scripted {
    use super::Confirm;
    use crate::error::Error;

    /// A prompt with its answer decided in advance.
    #[derive(Debug)]
    pub(crate) struct ScriptedConfirm {
        reply: Option<bool>,
        pub(crate) questions: Vec<String>,
    }

    impl ScriptedConfirm {
        pub(crate) const fn approving() -> Self {
            Self::answering(Some(true))
        }

        pub(crate) const fn declining() -> Self {
            Self::answering(Some(false))
        }

        /// Nobody is there to answer, as when standard input is not a terminal.
        pub(crate) const fn unattended() -> Self {
            Self::answering(None)
        }

        const fn answering(reply: Option<bool>) -> Self {
            Self {
                reply,
                questions: Vec::new(),
            }
        }
    }

    impl Confirm for ScriptedConfirm {
        fn confirm(&mut self, question: &str) -> Result<bool, Error> {
            self.questions.push(question.to_owned());
            self.reply.ok_or(Error::ConfirmationRequired)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{recording::RecordingReporter, scripted::ScriptedConfirm, *};

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

    #[test]
    fn an_answer_given_on_the_command_line_is_not_asked_for() {
        let mut confirm = TermConfirm::new(true);

        assert!(
            confirm
                .confirm("remove everything?")
                .expect("--yes answers itself")
        );
    }

    #[test]
    fn a_prompt_with_nobody_to_answer_it_fails() {
        // Standard input is not a terminal under the test harness, and no
        // `--yes` was given, so there is no answer to be had.
        let mut confirm = TermConfirm::new(false);

        let error = confirm
            .confirm("remove everything?")
            .expect_err("nobody can answer");

        assert!(matches!(error, Error::ConfirmationRequired), "{error:?}");
        assert!(error.to_string().contains("--yes"), "{error}");
    }

    #[test]
    fn a_scripted_prompt_records_what_it_was_asked() {
        let mut confirm = ScriptedConfirm::declining();

        assert!(!confirm.confirm("remove everything?").expect("an answer"));
        assert_eq!(confirm.questions, ["remove everything?"]);
    }
}
