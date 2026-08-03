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

/// The site's response to a submitted answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The answer was accepted.
    Correct,
    /// The answer was rejected.
    Incorrect,
    /// The answer was not accepted and a wait is required before retrying.
    ///
    /// The site does not distinguish "wrong, and you must wait" from "you
    /// answered too recently", so neither does this.
    NotAccepted {
        /// How long the site says to wait, as it phrased it.
        wait: String,
    },
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
    /// interpreted. A rejected answer is a [`Verdict`], not an error.
    fn submit(&self, puzzle: Puzzle, part: Part, answer: &str) -> Result<Verdict, ApiError>;
}

/// Errors produced while talking to Advent of Code.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    /// The request could not be completed.
    #[error("advent of code request failed: {0}")]
    Transport(String),
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
