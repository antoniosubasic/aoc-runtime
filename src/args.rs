use chrono::{Datelike, Local};
use clap::{Parser, ValueEnum};
use serde::{Serialize, Serializer};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]

// ensure longer names like "CSharp" are used without any dashes ("csharp" instead of "c-sharp")
#[clap(rename_all = "lowercase")]
pub enum Language {
    Rust,
    CSharp,
    Java,
    Python,
}

impl fmt::Display for Language {
    // make the enum be formatted in all lowercase when converting to a string
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}

// make sure the language enum is serialized lowercase
fn serialize_language<S>(language: &Option<Language>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match language {
        Some(lang) => serializer.serialize_str(&lang.to_string()),
        None => serializer.serialize_none(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Mode {
    Run,
    Init,
    Path,
    Code,
    Url,
}

#[derive(Parser, Serialize)]
pub struct Args {
    #[arg(
        short,
        long,
        
        // default to current year, if month is december, else previous year
        default_value_t = Local::now().year() - (Local::now().month() < 12) as i32,

        // allow years from 2015 to current year (inclusive)
        value_parser = clap::value_parser!(i32).range(2015..=(Local::now().year() as i64 - (Local::now().month() < 12) as i64))
    )]
    pub year: i32,

    #[arg(
        short,
        long,

        // default to current day, if month is december, else 1
        default_value_t = if Local::now().month() == 12 { Local::now().day() } else { 1 },

        // allow days from 1 to 25 (inclusive)
        value_parser = clap::value_parser!(u32).range(1..=25)
    )]
    pub day: u32,

    #[serde(serialize_with = "serialize_language")]
    #[arg(
        short,
        long,

        // make the language parameter required for all modes except "url"
        required_if_eq("mode", "run"),
        required_if_eq("mode", "init"),
        required_if_eq("mode", "path"),
        required_if_eq("mode", "code")
    )]
    pub language: Option<Language>,

    #[serde(skip)]
    #[arg(
        value_enum,
        default_value_t = Mode::Run
    )]
    pub mode: Mode,
}
