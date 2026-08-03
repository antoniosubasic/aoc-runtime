//! The real Advent of Code client.
//!
//! This is the only module that mentions `aoc_api` or `tokio`. The upstream
//! API is asynchronous while this tool performs a handful of strictly
//! sequential requests, so a current-thread runtime is driven to completion
//! here and the rest of the crate stays synchronous.

use super::{AocClient, ApiError, Verdict, throttle::Throttle};
use crate::puzzle::{Part, Puzzle};
use aoc_api::SubmitAnswerError;
use std::path::PathBuf;
use tokio::runtime::{Builder, Runtime};

/// A client backed by `adventofcode.com`.
///
/// Every request passes through a [`Throttle`] first, so this type is the one
/// place in the crate where the site is actually contacted and the one place
/// where the rate of contact is enforced.
#[derive(Debug)]
pub struct LiveClient {
    cookie: String,
    runtime: Runtime,
    throttle: Throttle,
}

impl LiveClient {
    /// Creates a client authenticated with the given session cookie, pacing
    /// its requests using state kept in `state_dir`.
    ///
    /// # Errors
    ///
    /// Returns [`ApiError::Transport`] if the runtime cannot be started.
    pub fn new(cookie: &str, state_dir: impl Into<PathBuf>) -> Result<Self, ApiError> {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|source| ApiError::Transport(source.to_string()))?;

        Ok(Self {
            cookie: cookie.to_owned(),
            runtime,
            throttle: Throttle::new(state_dir),
        })
    }
}

impl AocClient for LiveClient {
    fn fetch_input(&self, puzzle: Puzzle) -> Result<String, ApiError> {
        self.throttle.acquire();

        self.runtime.block_on(async {
            aoc_api::get_input_text(&self.cookie, puzzle.year.get(), puzzle.day.get())
                .await
                .map_err(|error| ApiError::Transport(error.to_string()))
        })
    }

    fn submit(&self, puzzle: Puzzle, part: Part, answer: &str) -> Result<Verdict, ApiError> {
        self.throttle.acquire();

        self.runtime.block_on(async {
            let submission = aoc_api::submit_answer_explicit_error(
                &self.cookie,
                puzzle.year.get(),
                puzzle.day.get(),
                part.number(),
                answer,
            )
            .await;

            match submission {
                Ok(true) => Ok(Verdict::Correct),
                Ok(false) => Ok(Verdict::Incorrect),
                Err(SubmitAnswerError::Cooldown(wait)) => Ok(Verdict::NotAccepted { wait }),
                Err(SubmitAnswerError::Unknown(message)) => Err(ApiError::Unexpected(message)),
                Err(SubmitAnswerError::Other(message)) => Err(ApiError::Transport(message)),
            }
        })
    }
}
