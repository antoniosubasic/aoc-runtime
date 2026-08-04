//! The Advent of Code boundary.
//!
//! Everything that talks to `adventofcode.com` goes through [`AocClient`], so
//! the submission logic can be exercised without a network - and so that a run
//! without a session cookie provably cannot make a request, because there is no
//! client to call.

pub mod cache;
pub mod live;
pub mod throttle;

use crate::puzzle::{Part, Puzzle};
use std::{fmt, time::Duration};

/// Which way a rejected answer was wrong, when the site says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hint {
    /// The answer is larger than the right one.
    TooHigh,
    /// The answer is smaller than the right one.
    TooLow,
}

impl fmt::Display for Hint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooHigh => f.write_str("too high"),
            Self::TooLow => f.write_str("too low"),
        }
    }
}

/// The site's response to a submitted answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The answer was accepted, or is the one already accepted for a part that
    /// was solved earlier.
    Correct,

    /// The answer was rejected.
    Incorrect {
        /// Which way it was wrong, when the site says.
        hint: Option<Hint>,
        /// How long the site asks you to wait before trying again, when it
        /// says.
        wait: Option<Duration>,
    },

    /// Nothing was judged, because an answer was submitted too recently. This
    /// answer still has to be sent again once the wait is over.
    Cooldown {
        /// How much of the wait is left.
        wait: Duration,
    },

    /// The site was not asking for an answer to that part - either part one is
    /// still unsolved, or the part was never a question, which is day 25's
    /// second star.
    WrongLevel,
}

impl Verdict {
    /// Whether the site judged the answer at all.
    ///
    /// A cooldown, or a part the site is not asking about, leaves an answer
    /// neither right nor wrong.
    #[must_use]
    pub const fn is_judged(&self) -> bool {
        matches!(self, Self::Correct | Self::Incorrect { .. })
    }

    /// How long to wait before another answer can be judged, when the site
    /// asked for a wait at all.
    #[must_use]
    pub const fn wait(&self) -> Option<Duration> {
        match self {
            Self::Incorrect { wait, .. } => *wait,
            Self::Cooldown { wait } => Some(*wait),
            Self::Correct | Self::WrongLevel => None,
        }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Correct => f.write_str("that's the right answer"),
            Self::Incorrect { hint, wait } => {
                f.write_str("that's not the right answer")?;
                if let Some(hint) = hint {
                    write!(f, "; it is {hint}")?;
                }
                if let Some(wait) = wait {
                    write!(f, " (wait {} before trying again)", describe(*wait))?;
                }
                Ok(())
            }
            Self::Cooldown { wait } => write!(
                f,
                "an answer was submitted too recently; {} left to wait",
                describe(*wait)
            ),
            Self::WrongLevel => f.write_str("advent of code is not asking for this answer"),
        }
    }
}

/// Renders a wait the way the site phrases it.
fn describe(wait: Duration) -> String {
    let total = wait.as_secs();
    let (hours, minutes, seconds) = (total / 3600, (total / 60) % 60, total % 60);

    let mut parts = Vec::new();
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes}m"));
    }
    if seconds > 0 || parts.is_empty() {
        parts.push(format!("{seconds}s"));
    }

    parts.join(" ")
}

/// Reads puzzle input and submits answers.
pub trait AocClient {
    /// Downloads the puzzle input.
    ///
    /// # Errors
    ///
    /// Returns [`ApiError`] if the request fails or the response cannot be
    /// interpreted.
    fn fetch_input(&self, puzzle: Puzzle) -> Result<String, ApiError>;

    /// Submits an answer and reports how the site responded.
    ///
    /// # Errors
    ///
    /// Returns [`ApiError`] if the request fails or the response cannot be
    /// interpreted. Everything the site had to say about the answer itself -
    /// including a refusal to judge it yet - is a [`Verdict`], not an error.
    fn submit(&self, puzzle: Puzzle, part: Part, answer: &str) -> Result<Verdict, ApiError>;
}

/// Errors produced while talking to Advent of Code.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    /// The request could not be completed.
    #[error("advent of code request failed: {0}")]
    Transport(String),

    /// The session cookie was missing, expired or refused.
    #[error("advent of code did not accept the session cookie; it is missing, expired or invalid")]
    Unauthorized,

    /// The puzzle has not unlocked yet.
    #[error("{puzzle} has not unlocked yet")]
    Locked {
        /// The puzzle that is still locked.
        puzzle: Puzzle,
    },

    /// The coordinates were refused before any request was made.
    ///
    /// [`Puzzle`] and the client underneath validate a puzzle by the same
    /// rules, so this means the two have drifted apart - not that anything was
    /// asked of the site.
    #[error("{puzzle} is not a puzzle the advent of code client accepts: {reason}")]
    Coordinates {
        /// The puzzle that was refused.
        puzzle: Puzzle,
        /// What the client said about it.
        reason: String,
    },

    /// The response was not one this tool understands.
    #[error("unexpected response from advent of code: {0}")]
    Unexpected(String),
}

#[cfg(test)]
pub(crate) mod fake {
    use super::{AocClient, ApiError, Puzzle, Verdict};
    use crate::puzzle::Part;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    #[derive(Debug, Default)]
    pub(crate) struct FakeClient {
        verdicts: RefCell<VecDeque<Result<Verdict, ApiError>>>,
        input: Option<String>,
        submitted: RefCell<Vec<(Puzzle, Part, String)>>,
    }

    impl FakeClient {
        pub(crate) fn new() -> Self {
            Self::default()
        }

        pub(crate) fn with_input(input: &str) -> Self {
            Self {
                input: Some(input.to_owned()),
                ..Self::default()
            }
        }

        pub(crate) fn push(&self, verdict: Verdict) -> &Self {
            self.verdicts.borrow_mut().push_back(Ok(verdict));
            self
        }

        pub(crate) fn submitted(&self) -> Vec<(Puzzle, Part, String)> {
            self.submitted.borrow().clone()
        }
    }

    impl AocClient for FakeClient {
        fn fetch_input(&self, _puzzle: Puzzle) -> Result<String, ApiError> {
            self.input
                .clone()
                .ok_or_else(|| ApiError::Transport("no input configured".to_owned()))
        }

        fn submit(&self, puzzle: Puzzle, part: Part, answer: &str) -> Result<Verdict, ApiError> {
            self.submitted
                .borrow_mut()
                .push((puzzle, part, answer.to_owned()));

            self.verdicts
                .borrow_mut()
                .pop_front()
                .unwrap_or(Ok(Verdict::Correct))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_judged_answer_is_right_or_wrong() {
        assert!(Verdict::Correct.is_judged());
        assert!(
            Verdict::Incorrect {
                hint: None,
                wait: None
            }
            .is_judged()
        );
        assert!(
            !Verdict::Cooldown {
                wait: Duration::from_secs(30)
            }
            .is_judged()
        );
        assert!(!Verdict::WrongLevel.is_judged());
    }

    #[test]
    fn a_wait_is_reported_however_the_site_phrased_the_refusal() {
        assert_eq!(Verdict::Correct.wait(), None);
        assert_eq!(Verdict::WrongLevel.wait(), None);
        assert_eq!(
            Verdict::Incorrect {
                hint: None,
                wait: Some(Duration::from_secs(60)),
            }
            .wait(),
            Some(Duration::from_secs(60))
        );
        assert_eq!(
            Verdict::Cooldown {
                wait: Duration::from_secs(270)
            }
            .wait(),
            Some(Duration::from_secs(270))
        );
    }

    #[test]
    fn a_verdict_reads_like_the_site_phrases_it() {
        assert_eq!(Verdict::Correct.to_string(), "that's the right answer");
        assert_eq!(
            Verdict::Incorrect {
                hint: Some(Hint::TooHigh),
                wait: Some(Duration::from_secs(60)),
            }
            .to_string(),
            "that's not the right answer; it is too high (wait 1m before trying again)"
        );
        assert_eq!(
            Verdict::Incorrect {
                hint: None,
                wait: None
            }
            .to_string(),
            "that's not the right answer"
        );
        assert_eq!(
            Verdict::Cooldown {
                wait: Duration::from_secs(270)
            }
            .to_string(),
            "an answer was submitted too recently; 4m 30s left to wait"
        );
    }

    #[test]
    fn a_wait_is_written_in_the_largest_units_it_fills() {
        assert_eq!(describe(Duration::ZERO), "0s");
        assert_eq!(describe(Duration::from_secs(45)), "45s");
        assert_eq!(describe(Duration::from_secs(60)), "1m");
        assert_eq!(describe(Duration::from_secs(270)), "4m 30s");
        assert_eq!(describe(Duration::from_secs(3661)), "1h 1m 1s");
    }
}
