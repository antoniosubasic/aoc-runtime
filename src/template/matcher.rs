//! Recovering template parameters from the current working directory.
//!
//! The pattern is generated from the parsed segments **in template order**:
//! everything after each placeholder is wrapped in an optional group, so
//! standing anywhere along the template's path recovers the parameters up to
//! that point, whatever order the placeholders appear in.

use super::{Segment, TemplateError};
use crate::{
    language::Language,
    puzzle::{Day, Year},
};
use regex::Regex;
use std::path::Path;

/// Parameters recovered from a directory path. Every field is best-effort:
/// only explicit command line arguments produce hard errors.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Detected {
    /// The detected year, if the path contained a valid one.
    pub year: Option<Year>,
    /// The detected day, if the path contained a valid one.
    pub day: Option<Day>,
    /// The detected language, if the path contained one.
    pub language: Option<Language>,
}

/// A compiled matcher for one template.
#[derive(Debug, Clone)]
pub struct CwdMatcher {
    pattern: Regex,
}

impl CwdMatcher {
    /// Compiles a matcher for the given segments.
    ///
    /// # Errors
    ///
    /// Returns [`TemplateError::Regex`] if the generated pattern is rejected.
    pub fn build(segments: &[Segment]) -> Result<Self, TemplateError> {
        Ok(Self {
            pattern: Regex::new(&build_pattern(segments))?,
        })
    }

    /// The generated regular expression, exposed for diagnostics and tests.
    #[must_use]
    pub fn pattern(&self) -> &str {
        self.pattern.as_str()
    }

    /// Recovers whatever parameters the path reveals.
    #[must_use]
    pub fn detect(&self, directory: &Path) -> Detected {
        let path = directory.to_string_lossy();
        let path = path.trim_end_matches(['/', '\\']);

        let Some(captures) = self.pattern.captures(path) else {
            return Detected::default();
        };

        let capture = |name| captures.name(name).map(|m| m.as_str());

        Detected {
            year: capture("year")
                .and_then(|v| v.parse().ok())
                .and_then(Year::new),
            day: capture("day")
                .and_then(|v| v.parse().ok())
                .and_then(Day::new),
            language: capture("language").and_then(|v| v.parse().ok()),
        }
    }
}

fn build_pattern(segments: &[Segment]) -> String {
    let mut chunks: Vec<String> = Vec::new();
    let mut pending = String::new();
    let mut seen = Seen::default();

    for segment in segments {
        match segment {
            Segment::Literal(text) => pending.push_str(&regex::escape(text)),
            Segment::Year | Segment::Day { .. } | Segment::Language => {
                pending.push_str(&placeholder_pattern(segment, &mut seen));
                chunks.push(std::mem::take(&mut pending));
            }
        }
    }

    let mut pattern = String::from("^");
    let mut groups = 0usize;

    for (index, chunk) in chunks.iter().enumerate() {
        if index > 0 {
            pattern.push_str("(?:");
            groups += 1;
        }
        pattern.push_str(chunk);
    }

    if !pending.is_empty() {
        pattern.push_str("(?:");
        groups += 1;
        pattern.push_str(&pending);
    }

    for _ in 0..groups {
        pattern.push_str(")?");
    }

    pattern.push_str(r"(?:[/\\].*)?$");
    pattern
}

#[derive(Default)]
struct Seen {
    year: bool,
    day: bool,
    language: bool,
}

fn placeholder_pattern(segment: &Segment, seen: &mut Seen) -> String {
    let (first, name, body) = match segment {
        Segment::Year => (
            !std::mem::replace(&mut seen.year, true),
            "year",
            r"\d{4}".to_owned(),
        ),
        Segment::Day { .. } => (
            !std::mem::replace(&mut seen.day, true),
            "day",
            r"\d{1,2}".to_owned(),
        ),
        Segment::Language => (
            !std::mem::replace(&mut seen.language, true),
            "language",
            language_alternation(),
        ),
        Segment::Literal(_) => return String::new(),
    };

    if first {
        format!("(?<{name}>{body})")
    } else {
        format!("(?:{body})")
    }
}

fn language_alternation() -> String {
    let mut names: Vec<&str> = Language::ALL
        .iter()
        .map(|language| language.name())
        .collect();
    names.sort_unstable_by_key(|name| (std::cmp::Reverse(name.len()), *name));
    names.join("|")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::Template;

    fn matcher(source: &str) -> CwdMatcher {
        Template::parse(source)
            .expect("template should parse")
            .matcher()
            .expect("pattern should compile")
    }

    fn detect(source: &str, cwd: &str) -> Detected {
        matcher(source).detect(Path::new(cwd))
    }

    fn triple(detected: Detected) -> (Option<u16>, Option<u8>, Option<Language>) {
        (
            detected.year.map(Year::get),
            detected.day.map(Day::get),
            detected.language,
        )
    }

    const CANONICAL: &str = "/root/{{year}}/day{{pad day}}/{{language}}";

    #[test]
    fn generates_an_anchored_pattern_with_nested_optional_groups() {
        assert_eq!(
            matcher(CANONICAL).pattern(),
            r"^/root/(?<year>\d{4})(?:/day(?<day>\d{1,2})(?:/(?<language>csharp|python|java|rust))?)?(?:[/\\].*)?$"
        );
    }

    #[test]
    fn recovers_every_parameter_from_a_full_path() {
        assert_eq!(
            triple(detect(CANONICAL, "/root/2024/day07/rust")),
            (Some(2024), Some(7), Some(Language::Rust))
        );
    }

    #[test]
    fn recovers_partial_parameters_from_a_prefix() {
        assert_eq!(triple(detect(CANONICAL, "/root")), (None, None, None));
        assert_eq!(
            triple(detect(CANONICAL, "/root/2024")),
            (Some(2024), None, None)
        );
        assert_eq!(
            triple(detect(CANONICAL, "/root/2024/day07")),
            (Some(2024), Some(7), None)
        );
    }

    #[test]
    fn recovers_parameters_from_a_deeper_directory() {
        assert_eq!(
            triple(detect(CANONICAL, "/root/2024/day07/rust/src/bin")),
            (Some(2024), Some(7), Some(Language::Rust))
        );
    }

    #[test]
    fn two_digit_days_are_not_truncated() {
        for (day, expected) in [
            (1, 1),
            (5, 5),
            (9, 9),
            (10, 10),
            (15, 15),
            (19, 19),
            (25, 25),
        ] {
            let padded = format!("/root/2024/day{day:02}/rust");
            let plain = format!("/root/2024/day{day}/rust");

            assert_eq!(
                triple(detect(CANONICAL, &padded)).1,
                Some(expected),
                "{padded}"
            );
            assert_eq!(
                triple(detect("/root/{{year}}/day{{day}}/{{language}}", &plain)).1,
                Some(expected),
                "{plain}"
            );
        }
    }

    #[test]
    fn out_of_range_values_are_dropped_individually() {
        assert_eq!(
            triple(detect(CANONICAL, "/root/2024/day26/rust")),
            (Some(2024), None, Some(Language::Rust))
        );
        assert_eq!(
            triple(detect(CANONICAL, "/root/2024/day00/rust")),
            (Some(2024), None, Some(Language::Rust))
        );
        assert_eq!(
            triple(detect(CANONICAL, "/root/1999/day07/rust")),
            (None, Some(7), Some(Language::Rust))
        );
    }

    #[test]
    fn unrelated_directories_yield_nothing() {
        assert_eq!(triple(detect(CANONICAL, "/tmp")), (None, None, None));
        assert_eq!(
            triple(detect(CANONICAL, "/elsewhere/2024/day07/rust")),
            (None, None, None)
        );
    }

    #[test]
    fn a_broken_middle_segment_stops_detection_there() {
        assert_eq!(
            triple(detect(CANONICAL, "/root/2024/scratch/rust")),
            (Some(2024), None, None)
        );
    }

    #[test]
    fn partial_matching_follows_template_order_not_a_fixed_order() {
        let reordered = "/root/{{language}}/{{year}}/day{{day}}";

        assert_eq!(
            triple(detect(reordered, "/root/rust")),
            (None, None, Some(Language::Rust))
        );
        assert_eq!(
            triple(detect(reordered, "/root/rust/2024")),
            (Some(2024), None, Some(Language::Rust))
        );
        assert_eq!(
            triple(detect(reordered, "/root/rust/2024/day7")),
            (Some(2024), Some(7), Some(Language::Rust))
        );
    }

    #[test]
    fn a_trailing_literal_is_optional() {
        let with_suffix = "/root/{{year}}/day{{pad day}}/{{language}}/solution";

        assert_eq!(
            triple(detect(with_suffix, "/root/2024/day07/rust")),
            (Some(2024), Some(7), Some(Language::Rust))
        );
        assert_eq!(
            triple(detect(with_suffix, "/root/2024/day07/rust/solution")),
            (Some(2024), Some(7), Some(Language::Rust))
        );
    }

    #[test]
    fn spaced_out_placeholders_still_detect() {
        assert_eq!(
            triple(detect(
                "/root/{{ year }}/day{{ pad day }}/{{ language }}",
                "/root/2024/day07/java"
            )),
            (Some(2024), Some(7), Some(Language::Java))
        );
    }

    #[test]
    fn literal_regex_metacharacters_are_escaped() {
        let dotted = "/root/a.c/{{year}}/day{{pad day}}";

        assert_eq!(triple(detect(dotted, "/root/a.c/2024/day07")).0, Some(2024));
        assert_eq!(triple(detect(dotted, "/root/abc/2024/day07")).0, None);
    }

    #[test]
    fn trailing_separators_are_ignored() {
        assert_eq!(
            triple(detect(CANONICAL, "/root/2024/day07/rust/")),
            (Some(2024), Some(7), Some(Language::Rust))
        );
    }

    #[test]
    fn every_language_is_recognised() {
        for language in Language::ALL {
            let cwd = format!("/root/2024/day07/{}", language.name());
            assert_eq!(triple(detect(CANONICAL, &cwd)).2, Some(*language), "{cwd}");
        }
    }

    #[test]
    fn matching_is_case_sensitive() {
        assert_eq!(
            triple(detect(CANONICAL, "/Root/2024/day07/rust")),
            (None, None, None)
        );
        assert_eq!(triple(detect(CANONICAL, "/root/2024/day07/Rust")).2, None);
    }

    #[test]
    fn repeated_placeholders_capture_once() {
        let repeated = "/root/{{year}}/day{{pad day}}/{{year}}";

        assert_eq!(
            triple(detect(repeated, "/root/2024/day07/2024")),
            (Some(2024), Some(7), None)
        );
    }
}
