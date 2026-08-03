//! Path templates.
//!
//! A template such as `~/projects/aoc/{{year}}/day{{pad day}}/{{language}}` is
//! parsed once into [`Segment`]s and then used in both directions: rendered
//! into a project path, and compiled into a regex that recovers the parameters
//! from the current working directory.

pub mod matcher;

use crate::{
    language::Language,
    puzzle::{Day, Year},
};
use std::{fmt, mem, path::PathBuf, str::FromStr};

/// One piece of a parsed template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Text copied through verbatim.
    Literal(String),
    /// The `{{year}}` placeholder.
    Year,
    /// The `{{day}}` or `{{pad day}}` placeholder.
    Day {
        /// Whether the day is zero-padded to two digits.
        padded: bool,
    },
    /// The `{{language}}` placeholder.
    Language,
}

/// The values substituted into a template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Params {
    /// The event year.
    pub year: Year,
    /// The puzzle day.
    pub day: Day,
    /// The solution language.
    pub language: Language,
}

/// A parsed path template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    segments: Vec<Segment>,
}

impl Template {
    /// Parses a template, validating its placeholders.
    ///
    /// Whitespace inside the braces is insignificant, so `{{pad day}}`,
    /// `{{ pad day }}` and `{{  pad   day }}` are the same placeholder.
    ///
    /// # Errors
    ///
    /// Returns [`TemplateError`] if a placeholder is unclosed or unrecognised,
    /// or if the template is missing the year or day placeholder - both are
    /// always substituted, so a template without them can never render.
    ///
    /// ```
    /// use aoc_runtime::template::{Segment, Template};
    ///
    /// let template = Template::parse("/aoc/{{year}}/day{{ pad day }}")?;
    /// assert_eq!(template.segments(), [
    ///     Segment::Literal("/aoc/".into()),
    ///     Segment::Year,
    ///     Segment::Literal("/day".into()),
    ///     Segment::Day { padded: true },
    /// ]);
    /// # Ok::<(), aoc_runtime::template::TemplateError>(())
    /// ```
    pub fn parse(source: &str) -> Result<Self, TemplateError> {
        let mut segments = Vec::new();
        let mut literal = String::new();
        let mut rest = source;
        let mut offset = 0usize;

        while let Some((before, after_open)) = rest.split_once("{{") {
            literal.push_str(before);
            let open_at = offset + before.len();

            let Some((name, after_close)) = after_open.split_once("}}") else {
                return Err(TemplateError::Unclosed { offset: open_at });
            };

            let segment = parse_placeholder(name, open_at)?;
            if !literal.is_empty() {
                segments.push(Segment::Literal(mem::take(&mut literal)));
            }
            segments.push(segment);

            offset = open_at + name.len() + 4;
            rest = after_close;
        }

        literal.push_str(rest);
        if !literal.is_empty() {
            segments.push(Segment::Literal(literal));
        }

        let template = Self { segments };
        for (which, present) in [
            ("year", template.contains(&Segment::Year)),
            ("day", template.day_padding().is_some()),
        ] {
            if !present {
                return Err(TemplateError::MissingPlaceholder { which });
            }
        }

        Ok(template)
    }

    /// The parsed segments, in template order.
    #[must_use]
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Renders the template into a concrete project path.
    ///
    /// Infallible: [`Params`] carries validated values, and parsing has already
    /// rejected templates that cannot be rendered.
    #[must_use]
    pub fn render(&self, params: Params) -> PathBuf {
        let mut rendered = String::with_capacity(64);

        for segment in &self.segments {
            match segment {
                Segment::Literal(text) => rendered.push_str(text),
                Segment::Year => rendered.push_str(&params.year.get().to_string()),
                Segment::Day { padded } => {
                    if *padded && params.day.get() < 10 {
                        rendered.push('0');
                    }
                    rendered.push_str(&params.day.get().to_string());
                }
                Segment::Language => rendered.push_str(params.language.name()),
            }
        }

        PathBuf::from(rendered)
    }

    /// Compiles the matcher that recovers parameters from a directory path.
    ///
    /// # Errors
    ///
    /// Returns [`TemplateError::Regex`] if the generated pattern is rejected,
    /// which in practice only happens for pathologically large templates.
    pub fn matcher(&self) -> Result<matcher::CwdMatcher, TemplateError> {
        matcher::CwdMatcher::build(&self.segments)
    }

    /// Whether the template mentions the language placeholder.
    #[must_use]
    pub fn has_language(&self) -> bool {
        self.contains(&Segment::Language)
    }

    fn contains(&self, wanted: &Segment) -> bool {
        self.segments.iter().any(|segment| segment == wanted)
    }

    fn day_padding(&self) -> Option<bool> {
        self.segments.iter().find_map(|segment| match segment {
            Segment::Day { padded } => Some(*padded),
            _ => None,
        })
    }
}

impl FromStr for Template {
    type Err = TemplateError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        Self::parse(source)
    }
}

impl fmt::Display for Template {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for segment in &self.segments {
            match segment {
                Segment::Literal(text) => f.write_str(text)?,
                Segment::Year => f.write_str("{{year}}")?,
                Segment::Day { padded: false } => f.write_str("{{day}}")?,
                Segment::Day { padded: true } => f.write_str("{{pad day}}")?,
                Segment::Language => f.write_str("{{language}}")?,
            }
        }
        Ok(())
    }
}

fn parse_placeholder(raw: &str, offset: usize) -> Result<Segment, TemplateError> {
    let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");

    match name.as_str() {
        "year" => Ok(Segment::Year),
        "day" => Ok(Segment::Day { padded: false }),
        "pad day" => Ok(Segment::Day { padded: true }),
        "language" => Ok(Segment::Language),
        _ => Err(TemplateError::UnknownPlaceholder { name, offset }),
    }
}

/// Errors produced while parsing or compiling a template.
#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    /// A `{{` was never closed.
    #[error("unclosed `{{{{` at position {offset}")]
    Unclosed {
        /// Byte offset of the opening braces.
        offset: usize,
    },
    /// A placeholder name is not recognised.
    #[error(
        "unknown placeholder `{{{{{name}}}}}` at position {offset} \
         (expected `year`, `day`, `pad day` or `language`)"
    )]
    UnknownPlaceholder {
        /// The normalised placeholder name.
        name: String,
        /// Byte offset of the opening braces.
        offset: usize,
    },
    /// A placeholder that must always be substituted is absent.
    #[error("template is missing the `{{{{{which}}}}}` placeholder")]
    MissingPlaceholder {
        /// The name of the missing placeholder.
        which: &'static str,
    },
    /// The generated matching pattern was rejected.
    #[error("template produced an invalid pattern")]
    Regex(#[from] regex::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template(source: &str) -> Template {
        Template::parse(source).expect("template should parse")
    }

    fn params(year: u16, day: u8, language: Language) -> Params {
        Params {
            year: Year::new(year).expect("valid year"),
            day: Day::new(day).expect("valid day"),
            language,
        }
    }

    #[test]
    fn parses_literals_and_placeholders_in_order() {
        assert_eq!(
            template("/aoc/{{year}}/day{{pad day}}/{{language}}").segments(),
            [
                Segment::Literal("/aoc/".to_owned()),
                Segment::Year,
                Segment::Literal("/day".to_owned()),
                Segment::Day { padded: true },
                Segment::Literal("/".to_owned()),
                Segment::Language,
            ]
        );
    }

    #[test]
    fn whitespace_inside_braces_is_insignificant() {
        let spaced = template("/aoc/{{ year }}/day{{  pad   day }}/{{\tlanguage\t}}");
        assert_eq!(
            spaced,
            template("/aoc/{{year}}/day{{pad day}}/{{language}}")
        );
    }

    #[test]
    fn distinguishes_padded_from_unpadded_days() {
        assert_eq!(
            template("{{year}}/{{day}}").segments().last(),
            Some(&Segment::Day { padded: false })
        );
        assert_eq!(
            template("{{year}}/{{pad day}}").segments().last(),
            Some(&Segment::Day { padded: true })
        );
    }

    #[test]
    fn rejects_unknown_placeholders() {
        let error = Template::parse("/aoc/{{year}}/{{month}}/{{day}}")
            .expect_err("month is not a placeholder");

        assert!(
            matches!(&error, TemplateError::UnknownPlaceholder { name, .. } if name == "month"),
            "got {error:?}"
        );
        assert!(error.to_string().contains("month"));
    }

    #[test]
    fn rejects_unclosed_placeholders() {
        let error = Template::parse("/aoc/{{year}}/day{{pad day").expect_err("brace is unclosed");
        assert!(
            matches!(error, TemplateError::Unclosed { .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn rejects_templates_that_can_never_render() {
        for (source, which) in [("/aoc/{{day}}", "year"), ("/aoc/{{year}}", "day")] {
            let error = Template::parse(source).expect_err("placeholder is missing");
            assert!(
                matches!(error, TemplateError::MissingPlaceholder { which: w } if w == which),
                "got {error:?}"
            );
        }
    }

    #[test]
    fn language_placeholder_is_optional() {
        let template = template("/aoc/{{year}}/day{{pad day}}");

        assert!(!template.has_language());
        assert_eq!(
            template.render(params(2024, 7, Language::Rust)),
            PathBuf::from("/aoc/2024/day07")
        );
    }

    #[test]
    fn renders_padded_and_unpadded_days() {
        let padded = template("/aoc/{{year}}/day{{pad day}}/{{language}}");
        let plain = template("/aoc/{{year}}/day{{day}}/{{language}}");

        assert_eq!(
            padded.render(params(2024, 7, Language::Rust)),
            PathBuf::from("/aoc/2024/day07/rust")
        );
        assert_eq!(
            padded.render(params(2015, 25, Language::CSharp)),
            PathBuf::from("/aoc/2015/day25/csharp")
        );
        assert_eq!(
            plain.render(params(2024, 7, Language::Python)),
            PathBuf::from("/aoc/2024/day7/python")
        );
        assert_eq!(
            plain.render(params(2024, 25, Language::Java)),
            PathBuf::from("/aoc/2024/day25/java")
        );
    }

    #[test]
    fn renders_every_occurrence_of_a_repeated_placeholder() {
        let template = template("/aoc/{{year}}/day{{pad day}}/{{year}}-{{pad day}}");

        assert_eq!(
            template.render(params(2024, 3, Language::Rust)),
            PathBuf::from("/aoc/2024/day03/2024-03")
        );
    }

    #[test]
    fn round_trips_through_display() {
        let source = "/aoc/{{year}}/day{{pad day}}/{{language}}";
        assert_eq!(template(source).to_string(), source);
    }
}
