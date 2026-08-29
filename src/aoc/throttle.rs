//! Keeping outbound requests to a polite rate.
//!
//! The Advent of Code [automation guidelines] ask that automated tools do not
//! hammer the site. This tool never polls and never runs on a schedule, so
//! every request it makes is one a person asked for; what needs controlling is
//! bursts. A single `aoc run --submit` would otherwise fetch an input and post
//! two answers back to back, and nothing would stop a shell loop from repeating
//! that as fast as the network allows.
//!
//! [`Throttle`] enforces a minimum gap between requests. The moment of the last
//! request is remembered in the state directory as well as in memory, so the
//! gap survives across separate invocations of the binary instead of only
//! holding within one.
//!
//! [automation guidelines]: https://www.reddit.com/r/adventofcode/wiki/faqs/automation

use std::{
    cell::Cell,
    fs,
    path::PathBuf,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// The smallest gap allowed between two outbound requests.
///
/// Long enough to break up bursts and to make an accidental loop self
/// limiting, short enough that submitting both parts of a puzzle in one run
/// stays comfortable. A person cannot solve a puzzle faster than this, so in
/// ordinary use the throttle never actually waits.
pub const MIN_INTERVAL: Duration = Duration::from_secs(5);

/// The file holding the time of the last request, sitting at the root of the
/// state directory rather than inside any of the caches.
///
/// Nothing may remove it: the gap it records is what makes the throttle hold
/// between one invocation and the next.
pub const LAST_REQUEST_FILE: &str = "last-request";

/// Wall-clock time, and the ability to wait for some of it.
///
/// Deliberately not [`crate::env::Clock`]: that answers "what day is it" when
/// resolving which puzzle to work on, whereas throttling needs instants and
/// sleeps.
pub trait Timer {
    /// The current wall-clock time.
    fn now(&self) -> SystemTime;

    /// Blocks the calling thread for the given duration.
    fn wait(&self, duration: Duration);
}

/// A timer backed by the system clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemTimer;

impl Timer for SystemTimer {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }

    fn wait(&self, duration: Duration) {
        thread::sleep(duration);
    }
}

/// Enforces a minimum gap between outbound requests.
#[derive(Debug)]
pub struct Throttle<T = SystemTimer> {
    path: PathBuf,
    timer: T,
    interval: Duration,
    last: Cell<Option<SystemTime>>,
}

impl Throttle {
    /// Creates a throttle keeping [`MIN_INTERVAL`] between requests, recording
    /// its state in the given state directory.
    #[must_use]
    pub fn new(state_dir: impl Into<PathBuf>) -> Self {
        Self::with_timer(state_dir, SystemTimer, MIN_INTERVAL)
    }
}

impl<T: Timer> Throttle<T> {
    /// Creates a throttle with an explicit timer and interval.
    #[must_use]
    pub fn with_timer(state_dir: impl Into<PathBuf>, timer: T, interval: Duration) -> Self {
        Self {
            path: state_dir.into().join(LAST_REQUEST_FILE),
            timer,
            interval,
            last: Cell::new(None),
        }
    }

    /// Blocks until another request may be sent, then records that one was.
    pub fn acquire(&self) {
        let now = self.timer.now();
        let wait = self.remaining(now);

        if !wait.is_zero() {
            self.timer.wait(wait);
        }

        self.record(now.checked_add(wait).unwrap_or(now));
    }

    /// How much longer a caller must wait before sending, as of `now`.
    fn remaining(&self, now: SystemTime) -> Duration {
        self.last_request()
            .and_then(|last| last.checked_add(self.interval))
            .and_then(|earliest| earliest.duration_since(now).ok())
            // A timestamp in the future - a corrupt file, or a clock that
            // moved - must not wedge the tool for longer than one interval.
            .map_or(Duration::ZERO, |wait| wait.min(self.interval))
    }

    /// When the most recent request was sent, by this process or an earlier
    /// one. An unreadable or malformed record simply means "no idea".
    fn last_request(&self) -> Option<SystemTime> {
        let persisted = fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| text.trim().parse::<u64>().ok())
            .and_then(|millis| UNIX_EPOCH.checked_add(Duration::from_millis(millis)));

        match (self.last.get(), persisted) {
            (Some(memory), Some(disk)) => Some(memory.max(disk)),
            (memory, disk) => memory.or(disk),
        }
    }

    /// Remembers when a request was sent.
    ///
    /// Persisting is best effort, matching the answer cache: a state directory
    /// that cannot be written costs the gap between separate invocations, but
    /// requests within this process are still spaced out.
    fn record(&self, at: SystemTime) {
        self.last.set(Some(at));

        let Ok(since_epoch) = at.duration_since(UNIX_EPOCH) else {
            return;
        };
        let Ok(millis) = u64::try_from(since_epoch.as_millis()) else {
            return;
        };
        let Some(parent) = self.path.parent() else {
            return;
        };

        if fs::create_dir_all(parent).is_ok() {
            let _ = fs::write(&self.path, millis.to_string());
        }
    }
}

#[cfg(test)]
pub(crate) mod fake {
    use super::{Duration, SystemTime, Timer};
    use std::cell::{Cell, RefCell};

    /// A timer that never really sleeps: waiting just moves it forward.
    #[derive(Debug)]
    pub(crate) struct FakeTimer {
        now: Cell<SystemTime>,
        waits: RefCell<Vec<Duration>>,
    }

    impl FakeTimer {
        pub(crate) fn new(now: SystemTime) -> Self {
            Self {
                now: Cell::new(now),
                waits: RefCell::new(Vec::new()),
            }
        }

        pub(crate) fn advance(&self, by: Duration) {
            self.now.set(self.now.get() + by);
        }

        /// Every duration the throttle asked to wait for, in order.
        pub(crate) fn waits(&self) -> Vec<Duration> {
            self.waits.borrow().clone()
        }
    }

    impl Timer for FakeTimer {
        fn now(&self) -> SystemTime {
            self.now.get()
        }

        fn wait(&self, duration: Duration) {
            self.waits.borrow_mut().push(duration);
            self.advance(duration);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{fake::FakeTimer, *};

    const INTERVAL: Duration = Duration::from_secs(5);

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn throttle(state_dir: impl Into<PathBuf>, now: u64) -> Throttle<FakeTimer> {
        Throttle::with_timer(state_dir, FakeTimer::new(at(now)), INTERVAL)
    }

    #[test]
    fn the_first_request_is_not_delayed() {
        let dir = tempfile::tempdir().expect("temp dir");
        let throttle = throttle(dir.path(), 1000);

        throttle.acquire();

        assert!(throttle.timer.waits().is_empty());
    }

    #[test]
    fn a_burst_is_spaced_out() {
        let dir = tempfile::tempdir().expect("temp dir");
        let throttle = throttle(dir.path(), 1000);

        throttle.acquire();
        throttle.acquire();
        throttle.acquire();

        assert_eq!(throttle.timer.waits(), [INTERVAL, INTERVAL]);
    }

    #[test]
    fn waiting_out_the_interval_costs_nothing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let throttle = throttle(dir.path(), 1000);

        throttle.acquire();
        throttle.timer.advance(INTERVAL);
        throttle.acquire();

        assert!(throttle.timer.waits().is_empty());
    }

    #[test]
    fn the_gap_is_honoured_across_invocations() {
        let dir = tempfile::tempdir().expect("temp dir");
        throttle(dir.path(), 1000).acquire();

        // A second process, two seconds later, reading the same state
        // directory: three of the five seconds are still outstanding.
        let next = throttle(dir.path(), 1002);
        next.acquire();

        assert_eq!(next.timer.waits(), [Duration::from_secs(3)]);
    }

    #[test]
    fn an_unwritable_state_directory_still_throttles_in_process() {
        let throttle = throttle("/proc/definitely-not-writable", 1000);

        throttle.acquire();
        throttle.acquire();

        assert_eq!(throttle.timer.waits(), [INTERVAL]);
    }

    #[test]
    fn a_timestamp_from_the_future_waits_at_most_one_interval() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(dir.path().join(LAST_REQUEST_FILE), "99000000000000").expect("seed timestamp");

        let throttle = throttle(dir.path(), 1000);
        throttle.acquire();

        assert_eq!(throttle.timer.waits(), [INTERVAL]);
    }

    #[test]
    fn an_unreadable_record_does_not_block_the_first_request() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(dir.path().join(LAST_REQUEST_FILE), "not a timestamp").expect("seed timestamp");

        let throttle = throttle(dir.path(), 1000);
        throttle.acquire();

        assert!(throttle.timer.waits().is_empty());
    }
}
