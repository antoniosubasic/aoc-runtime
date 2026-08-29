//! Validated puzzle coordinates.
//!
//! [`Year`] and [`Day`] have private fields and fallible constructors, so an
//! out-of-range value cannot reach path rendering or the Advent of Code API.
//! [`Puzzle`] checks the pairing on top of that: from [`Year::FIRST_SHORT`] on
//! the event stops after [`Day::LAST_SHORT`].

use std::fmt;

/// A validated Advent of Code year.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Year(u16);

impl Year {
    /// The first year Advent of Code ran.
    pub const FIRST: Self = Self(2015);
    /// The first shortened event. From 2025 on, Advent of Code publishes 12
    /// puzzles instead of 25.
    pub const FIRST_SHORT: Self = Self(2025);

    /// Creates a year, rejecting anything before [`Year::FIRST`].
    #[must_use]
    pub const fn new(year: u16) -> Option<Self> {
        if year >= Self::FIRST.0 {
            Some(Self(year))
        } else {
            None
        }
    }

    /// The last day this event publishes a puzzle on.
    #[must_use]
    pub const fn last_day(self) -> Day {
        if self.0 >= Self::FIRST_SHORT.0 {
            Day::LAST_SHORT
        } else {
            Day::LAST_FULL
        }
    }

    /// Whether this event has a puzzle on the given day.
    #[must_use]
    pub const fn has_day(self, day: Day) -> bool {
        day.0 <= self.last_day().0
    }

    /// The underlying value.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl fmt::Display for Year {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A validated Advent of Code day, always within `1..=25`.
///
/// The upper bound is the widest range any event has had; which days a
/// *particular* event has is [`Year::has_day`], enforced by [`Puzzle::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day(u8);

impl Day {
    /// The first puzzle day.
    pub const FIRST: Self = Self(1);
    /// The last puzzle day of events up to and including 2024.
    pub const LAST_FULL: Self = Self(25);
    /// The last puzzle day from [`Year::FIRST_SHORT`] on.
    pub const LAST_SHORT: Self = Self(12);

    /// Creates a day, rejecting anything outside `1..=25`.
    #[must_use]
    pub const fn new(day: u8) -> Option<Self> {
        if day >= Self::FIRST.0 && day <= Self::LAST_FULL.0 {
            Some(Self(day))
        } else {
            None
        }
    }

    /// Clamps an arbitrary calendar day into the year's puzzle range.
    ///
    /// December has 31 days but only 25 puzzles - 12 from
    /// [`Year::FIRST_SHORT`] on - so anything past the end clamps to the last
    /// day of that event.
    #[must_use]
    pub const fn clamped(year: Year, day: u32) -> Self {
        let last = year.last_day();

        if day < Self::FIRST.0 as u32 {
            Self::FIRST
        } else if day > last.0 as u32 {
            last
        } else {
            #[allow(clippy::cast_possible_truncation)]
            Self(day as u8)
        }
    }

    /// The underlying value.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Which half of a puzzle an answer belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Part {
    /// The first part.
    One,
    /// The second part, unlocked by solving the first.
    Two,
}

impl Part {
    /// The part number as understood by the Advent of Code API.
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::One => 1,
            Self::Two => 2,
        }
    }
}

impl fmt::Display for Part {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "part {}", self.number())
    }
}

/// A specific puzzle: one day of one year.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Puzzle {
    /// The event year.
    pub year: Year,
    /// The day within the event.
    pub day: Day,
}

impl Puzzle {
    /// Creates a puzzle, rejecting a day the event never published.
    ///
    /// Both coordinates are valid on their own, but not every pairing is: from
    /// [`Year::FIRST_SHORT`] on the event stops after [`Day::LAST_SHORT`].
    #[must_use]
    pub const fn new(year: Year, day: Day) -> Option<Self> {
        if year.has_day(day) {
            Some(Self { year, day })
        } else {
            None
        }
    }

    /// The canonical puzzle URL.
    #[must_use]
    pub fn url(self) -> String {
        format!("https://adventofcode.com/{}/day/{}", self.year, self.day)
    }

    /// The name this puzzle's files are kept under in the state directory, for
    /// example `2024-07`. Sorting the names sorts the puzzles.
    #[must_use]
    pub fn slug(self) -> String {
        format!("{}-{:02}", self.year.get(), self.day.get())
    }
}

impl fmt::Display for Puzzle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} day {}", self.year, self.day)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn year(year: u16) -> Year {
        Year::new(year).expect("valid year")
    }

    fn day(day: u8) -> Day {
        Day::new(day).expect("valid day")
    }

    #[test]
    fn year_rejects_years_before_the_first_event() {
        assert_eq!(Year::new(2014), None);
        assert_eq!(Year::new(2015).map(Year::get), Some(2015));
        assert_eq!(Year::new(2024).map(Year::get), Some(2024));
    }

    #[test]
    fn day_accepts_only_puzzle_days() {
        assert_eq!(Day::new(0), None);
        assert_eq!(Day::new(26), None);
        assert_eq!(Day::new(1).map(Day::get), Some(1));
        assert_eq!(Day::new(25).map(Day::get), Some(25));
    }

    #[test]
    fn events_from_2025_on_are_half_as_long() {
        assert_eq!(year(2015).last_day().get(), 25);
        assert_eq!(year(2024).last_day().get(), 25);
        assert_eq!(year(2025).last_day().get(), 12);
        assert_eq!(year(2030).last_day().get(), 12);

        assert!(year(2024).has_day(day(25)));
        assert!(year(2025).has_day(day(12)));
        assert!(!year(2025).has_day(day(13)));
        assert!(!year(2025).has_day(day(25)));
    }

    #[test]
    fn day_clamps_late_december_dates() {
        assert_eq!(Day::clamped(year(2024), 26).get(), 25);
        assert_eq!(Day::clamped(year(2024), 31).get(), 25);
        assert_eq!(Day::clamped(year(2024), 25).get(), 25);
        assert_eq!(Day::clamped(year(2024), 7).get(), 7);
        assert_eq!(Day::clamped(year(2024), 0).get(), 1);
    }

    #[test]
    fn day_clamps_to_the_end_of_a_shortened_event() {
        assert_eq!(Day::clamped(year(2025), 13).get(), 12);
        assert_eq!(Day::clamped(year(2025), 25).get(), 12);
        assert_eq!(Day::clamped(year(2025), 31).get(), 12);
        assert_eq!(Day::clamped(year(2025), 7).get(), 7);
    }

    #[test]
    fn puzzle_rejects_days_its_event_never_published() {
        assert!(Puzzle::new(year(2024), day(25)).is_some());
        assert!(Puzzle::new(year(2025), day(12)).is_some());
        assert_eq!(Puzzle::new(year(2025), day(13)), None);
        assert_eq!(Puzzle::new(year(2025), day(25)), None);
    }

    #[test]
    fn puzzle_renders_the_canonical_url() {
        let puzzle = Puzzle::new(year(2024), day(5)).expect("2024 has a day 5");

        assert_eq!(puzzle.url(), "https://adventofcode.com/2024/day/5");
    }

    #[test]
    fn parts_map_to_api_numbers() {
        assert_eq!(Part::One.number(), 1);
        assert_eq!(Part::Two.number(), 2);
        assert_eq!(Part::Two.to_string(), "part 2");
    }
}
