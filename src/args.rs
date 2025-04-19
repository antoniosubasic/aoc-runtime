use chrono::{Datelike, Local};
use clap::{Parser, ValueEnum};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Mode {
    Config,
    Run,
}

#[derive(Parser)]
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
        default_value_t = Local::now().day(),
        value_parser = clap::value_parser!(u32).range(1..=25)
    )]
    pub day: u32,

    #[arg(short, long)]
    pub language: Language,

    #[arg(
        value_enum,
        default_value_t = Mode::Run
    )]
    pub mode: Mode,
}
