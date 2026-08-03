//! The stdout protocol used to recognise puzzle answers.
//!
//! A solution communicates answers by printing one or two lines: the first is
//! part one, the second is part two. Anything else is treated as ordinary
//! program output and passed through untouched.

use crate::puzzle::Part;

/// The answers a solution printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answers {
    /// The part one answer.
    pub part1: String,
    /// The part two answer, if the solution printed one.
    pub part2: Option<String>,
}

impl Answers {
    /// Iterates over the answers together with the part they belong to.
    pub fn iter(&self) -> impl Iterator<Item = (Part, &str)> {
        std::iter::once((Part::One, self.part1.as_str()))
            .chain(self.part2.as_deref().map(|answer| (Part::Two, answer)))
    }
}

/// How a solution's standard output was interpreted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Output matched the answer protocol.
    Answers(Answers),
    /// Output did not match the protocol and should be printed verbatim.
    Raw,
}

/// Classifies a solution's standard output.
///
/// One or two non-blank lines are answers; zero, three or more lines - or any
/// blank line - is ordinary output.
///
/// ```
/// use aoc_runtime::answer::{classify, Outcome};
///
/// let Outcome::Answers(answers) = classify("1227\n23262\n") else {
///     panic!("two lines are answers");
/// };
/// assert_eq!(answers.part1, "1227");
/// assert_eq!(answers.part2.as_deref(), Some("23262"));
///
/// assert_eq!(classify("running...\n1227\n23262\n"), Outcome::Raw);
/// ```
#[must_use]
pub fn classify(stdout: &str) -> Outcome {
    let body = stdout.strip_suffix('\n').unwrap_or(stdout);
    if body.is_empty() {
        return Outcome::Raw;
    }

    let mut lines = body.split('\n').map(str::trim);
    let (Some(part1), part2, None) = (lines.next(), lines.next(), lines.next()) else {
        return Outcome::Raw;
    };

    if part1.is_empty() || part2.is_some_and(str::is_empty) {
        return Outcome::Raw;
    }

    Outcome::Answers(Answers {
        part1: part1.to_owned(),
        part2: part2.map(ToOwned::to_owned),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answers(stdout: &str) -> Option<Answers> {
        match classify(stdout) {
            Outcome::Answers(answers) => Some(answers),
            Outcome::Raw => None,
        }
    }

    fn parts(stdout: &str) -> Option<(String, Option<String>)> {
        answers(stdout).map(|a| (a.part1, a.part2))
    }

    #[test]
    fn one_line_is_part_one() {
        assert_eq!(parts("42\n"), Some(("42".to_owned(), None)));
    }

    #[test]
    fn two_lines_are_both_parts() {
        assert_eq!(
            parts("42\n99\n"),
            Some(("42".to_owned(), Some("99".to_owned())))
        );
    }

    #[test]
    fn a_missing_trailing_newline_still_yields_both_parts() {
        assert_eq!(
            parts("42\n99"),
            Some(("42".to_owned(), Some("99".to_owned())))
        );
        assert_eq!(parts("42"), Some(("42".to_owned(), None)));
    }

    #[test]
    fn multibyte_output_splits_on_character_boundaries() {
        assert_eq!(
            parts("ä→é\n42\n"),
            Some(("ä→é".to_owned(), Some("42".to_owned())))
        );
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        assert_eq!(
            parts("  42  \n\t99\t\n"),
            Some(("42".to_owned(), Some("99".to_owned())))
        );
    }

    #[test]
    fn carriage_returns_are_trimmed() {
        assert_eq!(
            parts("42\r\n99\r\n"),
            Some(("42".to_owned(), Some("99".to_owned())))
        );
    }

    #[test]
    fn three_or_more_lines_are_raw_output() {
        assert_eq!(classify("1\n2\n3\n"), Outcome::Raw);
        assert_eq!(classify("a\nb\nc\nd\n"), Outcome::Raw);
    }

    #[test]
    fn empty_and_blank_output_is_raw() {
        assert_eq!(classify(""), Outcome::Raw);
        assert_eq!(classify("\n"), Outcome::Raw);
        assert_eq!(classify("   \n"), Outcome::Raw);
        assert_eq!(classify("42\n\n"), Outcome::Raw);
        assert_eq!(classify("\n42\n"), Outcome::Raw);
    }

    #[test]
    fn iterates_parts_in_order() {
        let answers = Answers {
            part1: "a".to_owned(),
            part2: Some("b".to_owned()),
        };

        let collected: Vec<_> = answers.iter().collect();
        assert_eq!(collected, [(Part::One, "a"), (Part::Two, "b")]);

        let single = Answers {
            part1: "a".to_owned(),
            part2: None,
        };
        assert_eq!(single.iter().count(), 1);
    }
}
