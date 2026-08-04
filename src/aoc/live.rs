//! The real Advent of Code client.
//!
//! This is the only module that mentions `aoc_api` or `tokio`. The upstream
//! API is asynchronous while this tool performs a handful of strictly
//! sequential requests, so a current-thread runtime is driven to completion
//! here and the rest of the crate stays synchronous.
//!
//! Every endpoint upstream is a free function over an
//! [`aoc_api::http::Transport`], which is where the throttle sits: requests are
//! paced by the transport [`LiveClient`] holds rather than by the two calls
//! below, so a request this module does not make itself - the puzzle page
//! `submit` reads when a part turns out to be solved already - is paced like
//! any other.

use super::{AocClient, ApiError, Hint, Verdict, throttle::Throttle};
use crate::puzzle::{Part, Puzzle};
use aoc_api::{
    http::{ClientOptions, Request, ReqwestTransport, Response, Transport, TransportError},
    session,
};
use std::{error::Error as StdError, future::Future, path::PathBuf};
use tokio::runtime::{Builder, Runtime};

/// How this tool identifies itself in the `User-Agent` of every request.
///
/// The Advent of Code [automation guidelines] ask an automated tool to say what
/// it is and how to reach whoever wrote it. The address is spelled out rather
/// than read from `CARGO_PKG_AUTHORS`, because that field also holds a name an
/// HTTP header cannot carry.
///
/// [automation guidelines]: https://www.reddit.com/r/adventofcode/wiki/faqs/automation
pub const IDENTIFICATION: &str = concat!(
    env!("CARGO_PKG_REPOSITORY"),
    " v",
    env!("CARGO_PKG_VERSION"),
    " by antonio.subasic.public@gmail.com",
);

/// The transport every request this tool makes goes out through.
///
/// Wrapping the real transport rather than throttling at each call site means
/// there is one place a request can leave from, and it waits its turn first.
#[derive(Debug)]
struct ThrottledTransport {
    inner: ReqwestTransport,
    throttle: Throttle,
}

impl Transport for ThrottledTransport {
    fn execute(
        &self,
        request: Request,
    ) -> impl Future<Output = Result<Response, TransportError>> + Send {
        // The wait is taken when the request is asked for rather than when the
        // returned future is first polled, so nothing can be queued up past the
        // throttle. It blocks, which is what this whole client does: the
        // runtime is current-thread and has nothing else to run.
        self.throttle.acquire();
        self.inner.execute(request)
    }
}

/// A client backed by `adventofcode.com`.
///
/// This type is the one place in the crate where the site is actually
/// contacted, and - through the transport it holds - the one place where the
/// rate of contact is enforced.
#[derive(Debug)]
pub struct LiveClient {
    runtime: Runtime,
    transport: ThrottledTransport,
}

impl LiveClient {
    /// Creates a client authenticated with the given session cookie, pacing its
    /// requests using state kept in `state_dir`.
    ///
    /// # Errors
    ///
    /// Returns [`ApiError::Transport`] if the runtime cannot be started, or if
    /// the cookie cannot be sent in a header.
    pub fn new(cookie: &str, state_dir: impl Into<PathBuf>) -> Result<Self, ApiError> {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|source| ApiError::Transport(source.to_string()))?;

        let inner = ReqwestTransport::new(&ClientOptions::new(cookie, IDENTIFICATION))
            .map_err(|error| ApiError::Transport(chained(&error)))?;

        Ok(Self {
            runtime,
            transport: ThrottledTransport {
                inner,
                throttle: Throttle::new(state_dir),
            },
        })
    }
}

impl AocClient for LiveClient {
    fn fetch_input(&self, puzzle: Puzzle) -> Result<String, ApiError> {
        let coordinates = coordinates(puzzle)?;

        self.runtime.block_on(async {
            session::input_text(&self.transport, coordinates)
                .await
                .map_err(|error| translate(&error, puzzle))
        })
    }

    fn submit(&self, puzzle: Puzzle, part: Part, answer: &str) -> Result<Verdict, ApiError> {
        let coordinates = coordinates(puzzle)?;

        self.runtime.block_on(async {
            match session::submit(&self.transport, coordinates, level(part), answer).await {
                Ok(judged) => verdict(judged),
                // Nothing was judged, but that is an outcome of the submission
                // rather than a failure to make it.
                Err(aoc_api::Error::Cooldown { wait }) => Ok(Verdict::Cooldown { wait }),
                Err(error) => Err(translate(&error, puzzle)),
            }
        })
    }
}

/// The client's own coordinates for a puzzle.
fn coordinates(puzzle: Puzzle) -> Result<aoc_api::Puzzle, ApiError> {
    aoc_api::Puzzle::at(puzzle.year.get(), puzzle.day.get()).map_err(|reason| {
        ApiError::Coordinates {
            puzzle,
            reason: reason.to_string(),
        }
    })
}

/// The client's own name for a part.
const fn level(part: Part) -> aoc_api::Part {
    match part {
        Part::One => aoc_api::Part::One,
        Part::Two => aoc_api::Part::Two,
    }
}

/// This crate's reading of a judged submission.
fn verdict(judged: aoc_api::Verdict) -> Result<Verdict, ApiError> {
    match judged {
        aoc_api::Verdict::Correct | aoc_api::Verdict::AlreadyComplete { correct: true } => {
            Ok(Verdict::Correct)
        }
        aoc_api::Verdict::Incorrect { hint, wait } => Ok(Verdict::Incorrect {
            hint: recognised(hint),
            wait,
        }),
        // The part was solved with something else, so this answer is wrong -
        // and the site said so without being asked to judge it, so there is no
        // wait attached.
        aoc_api::Verdict::AlreadyComplete { correct: false } => Ok(Verdict::Incorrect {
            hint: None,
            wait: None,
        }),
        aoc_api::Verdict::WrongLevel => Ok(Verdict::WrongLevel),
        // A newer client may judge an answer in a way this one has never seen.
        // Saying so beats guessing which of the four it resembles.
        judged => Err(ApiError::Unexpected(judged.to_string())),
    }
}

/// A hint this crate knows, or none: not knowing which way an answer is wrong
/// costs a line of advice rather than correctness.
const fn recognised(hint: Option<aoc_api::Hint>) -> Option<Hint> {
    match hint {
        Some(aoc_api::Hint::TooHigh) => Some(Hint::TooHigh),
        Some(aoc_api::Hint::TooLow) => Some(Hint::TooLow),
        _ => None,
    }
}

/// This crate's reading of a call that produced no answer.
fn translate(error: &aoc_api::Error, puzzle: Puzzle) -> ApiError {
    match error {
        aoc_api::Error::Unauthorized => ApiError::Unauthorized,
        // The client names the puzzle it was handed; this names the one the
        // user asked for, which is the same puzzle either way.
        aoc_api::Error::Locked { .. } => ApiError::Locked { puzzle },
        aoc_api::Error::Parse(_) | aoc_api::Error::Puzzle(_) => {
            ApiError::Unexpected(chained(error))
        }
        error => ApiError::Transport(chained(error)),
    }
}

/// An error and its causes, flattened onto one line.
///
/// [`ApiError`] carries text rather than a boxed cause, so the reason a request
/// failed is folded in here instead of being dropped at the boundary.
fn chained(error: &dyn StdError) -> String {
    let mut message = error.to_string();
    let mut source = error.source();

    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }

    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::puzzle::{Day, Year};
    use std::time::Duration;

    fn puzzle() -> Puzzle {
        Puzzle::new(
            Year::new(2024).expect("valid year"),
            Day::new(7).expect("valid day"),
        )
        .expect("2024 has a day 7")
    }

    #[test]
    fn the_identification_is_one_a_request_can_actually_carry() {
        assert!(IDENTIFICATION.contains("aoc-runtime"), "{IDENTIFICATION}");
        assert!(
            ReqwestTransport::new(&ClientOptions::new("53616c7465645f5f", IDENTIFICATION)).is_ok(),
            "the user agent must not be one an http header rejects: {IDENTIFICATION}"
        );
    }

    #[test]
    fn coordinates_cross_to_the_client_unchanged() {
        assert_eq!(
            coordinates(puzzle()),
            Ok(aoc_api::Puzzle::at(2024, 7).expect("2024 has a day 7"))
        );
    }

    // Both crates decide for themselves how long an event runs. If they ever
    // disagree, the last day of a shortened event is where it shows first.
    #[test]
    fn the_last_day_of_a_shortened_event_crosses_too() {
        let puzzle = Puzzle::new(
            Year::new(2025).expect("valid year"),
            Day::new(12).expect("valid day"),
        )
        .expect("2025 has a day 12");

        assert_eq!(
            coordinates(puzzle),
            Ok(aoc_api::Puzzle::at(2025, 12).expect("2025 has a day 12"))
        );
    }

    #[test]
    fn parts_cross_as_the_levels_the_site_uses() {
        assert_eq!(level(Part::One).number(), 1);
        assert_eq!(level(Part::Two).number(), 2);
    }

    #[test]
    fn an_answer_already_accepted_counts_as_accepted() {
        assert_eq!(verdict(aoc_api::Verdict::Correct), Ok(Verdict::Correct));
        assert_eq!(
            verdict(aoc_api::Verdict::AlreadyComplete { correct: true }),
            Ok(Verdict::Correct)
        );
    }

    #[test]
    fn a_rejected_answer_keeps_the_hint_and_the_wait() {
        assert_eq!(
            verdict(aoc_api::Verdict::Incorrect {
                hint: Some(aoc_api::Hint::TooLow),
                wait: Some(Duration::from_secs(60)),
            }),
            Ok(Verdict::Incorrect {
                hint: Some(Hint::TooLow),
                wait: Some(Duration::from_secs(60)),
            })
        );
    }

    #[test]
    fn an_answer_that_differs_from_the_accepted_one_is_wrong() {
        assert_eq!(
            verdict(aoc_api::Verdict::AlreadyComplete { correct: false }),
            Ok(Verdict::Incorrect {
                hint: None,
                wait: None,
            })
        );
    }

    #[test]
    fn a_part_the_site_is_not_asking_about_is_not_a_failure() {
        assert_eq!(
            verdict(aoc_api::Verdict::WrongLevel),
            Ok(Verdict::WrongLevel)
        );
    }

    #[test]
    fn a_refused_cookie_is_reported_as_itself() {
        assert_eq!(
            translate(&aoc_api::Error::Unauthorized, puzzle()),
            ApiError::Unauthorized
        );
    }

    #[test]
    fn a_locked_puzzle_names_the_one_that_was_asked_for() {
        let error = aoc_api::Error::Locked {
            puzzle: aoc_api::Puzzle::at(2024, 7).expect("2024 has a day 7"),
        };

        assert_eq!(
            translate(&error, puzzle()),
            ApiError::Locked { puzzle: puzzle() }
        );
    }

    #[test]
    fn a_failed_request_keeps_the_reason_it_failed() {
        let error = aoc_api::Error::from(TransportError::Request {
            url: "https://adventofcode.com/2024/day/7/input".to_owned(),
            source: "connection reset".into(),
        });

        let translated = translate(&error, puzzle());

        assert!(matches!(translated, ApiError::Transport(_)), "{translated}");
        let message = translated.to_string();
        assert!(message.contains("/2024/day/7/input"), "{message}");
        assert!(message.contains("connection reset"), "{message}");
    }

    #[test]
    fn a_reply_the_client_could_not_read_is_not_a_transport_failure() {
        let error = aoc_api::Error::Parse(aoc_api::parse::ParseError::Submission {
            snippet: "Ho ho ho.".to_owned(),
        });

        assert!(
            matches!(translate(&error, puzzle()), ApiError::Unexpected(_)),
            "an unrecognised reply arrived, so the request itself succeeded"
        );
    }
}
