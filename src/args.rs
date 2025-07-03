use chrono::{Datelike, Local};
use clap::{Parser, ValueEnum};
use serde::{Serialize, Serializer};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
pub enum Language {
    Rust,
    CSharp,
    Java,
    Python,
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}

fn serialize_language<S>(language: &Language, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&language.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Mode {
    Config,
    Run,
}

#[derive(Parser, Serialize)]
pub struct Args {
    #[arg(
        short,
        long,
        default_value_t = Local::now().year() - (Local::now().month() < 12) as i32,
        value_parser = clap::value_parser!(i32).range(2015..=(Local::now().year() as i64 - (Local::now().month() < 12) as i64))
    )]
    pub year: i32,

    #[arg(
        short,
        long,
        default_value_t = if Local::now().month() == 12 { Local::now().day() } else { 1 },
        value_parser = clap::value_parser!(u32).range(1..=25)
    )]
    pub day: u32,

    #[serde(serialize_with = "serialize_language")]
    #[arg(short, long)]
    pub language: Language,

    #[serde(skip)]
    #[arg(
        value_enum,
        default_value_t = Mode::Run
    )]
    pub mode: Mode,
}
