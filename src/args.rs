use chrono::{Datelike, Local};
use clap::{Parser, ValueEnum};
use serde::{Serialize, Serializer};
use strum_macros::EnumIter;
use std::{fmt, process::Command};
use anyhow::Result;
use std::str::FromStr;
use strum::IntoEnumIterator;

use crate::{command, config::{Config, OptionalParameters}};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, EnumIter)]
#[clap(rename_all = "lowercase")] // ensure longer names like "CSharp" are used without any dashes ("csharp" instead of "c-sharp")
pub enum Language {
    Rust,
    CSharp,
    Java,
    Python,
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

impl fmt::Display for Language {
    // make the enum be formatted in all lowercase when converting to a string
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}

impl FromStr for Language {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for lang in Language::iter() {
            if lang.to_string() == s.to_lowercase() {
                return Ok(lang);
            }
        }

        Err(format!("unknown language: {}", s))
    }
}

impl Language {
    pub fn init_command(&self, config: &Config) -> Command {
        match *self {
            Language::Rust => command!("cargo", "init", "--bin", &config.project_path),
            Language::CSharp => command!(
                "dotnet",
                "new",
                "console",
                "--name",
                &config.project_path.file_name().unwrap().to_str().unwrap(),
                "--output",
                &config.project_path
            ),
            Language::Java => command!("touch", &config.project_path.join("Main.java")),
            Language::Python => command!("touch", &config.project_path.join("main.py")),
        }
    }

    pub fn build_command(&self, config: &Config) -> Option<Command> {
        match *self {
            Language::Rust => Some(
                command!(
                    "cargo",
                    "build",
                    "--release",
                    "--manifest-path",
                    &config.project_path.join("Cargo.toml")
                )
            ),
            Language::CSharp => Some(
                command!(
                    "dotnet",
                    "build",
                    &config.project_path
                )
            ),
            Language::Java => Some(
                command!(
                    "javac",
                    &config.project_path.join("Main.java")
                )
            ),
            Language::Python => None,
        }.map(|mut command| {
            command.current_dir(&config.project_path); 
            command
        })
    }

    pub fn run_command(&self, config: &Config) -> Command {
        let mut command = match *self {
            Language::Rust => command!(
                "cargo",
                "run",
                "--manifest-path",
                &config.project_path.join("Cargo.toml")
            ),
            Language::CSharp => command!(
                "dotnet",
                "run",
                "--project",
                &config.project_path
            ),
            Language::Java => command!(
                "java",
                "-cp",
                &config.project_path,
                "Main"
            ),
            Language::Python => command!(
                "python",
                &config.project_path.join("main.py")
            ),
        };
        command.current_dir(&config.project_path);
        command
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
        
        // allow years from 2015 to current year (inclusive)
        value_parser = clap::value_parser!(u16).range(2015..=(Local::now().year() as i64 - (Local::now().month() < 12) as i64))
    )]
    pub year: Option<u16>,

    #[arg(
        short,
        long,

        // allow days from 1 to 25 (inclusive)
        value_parser = clap::value_parser!(u8).range(1..=25)
    )]
    pub day: Option<u8>,

    #[serde(serialize_with = "serialize_language")]
    #[arg(short, long)]
    pub language: Option<Language>,

    #[serde(skip)]
    #[arg(
        value_enum,
        default_value_t = Mode::Run
    )]
    pub mode: Mode,
}

impl Args {
    pub fn build(&mut self, optional_parameters: OptionalParameters) {
        self.year = self.year
            .or(optional_parameters.year)
            .or(Some(Local::now().year() as u16 - (Local::now().month() < 12) as u16)); // default to current year, if month is december, else previous year

        self.day = self.day
            .or(optional_parameters.day)
            .or(Some(if Local::now().month() == 12 { Local::now().day() as u8 } else { 1 })); // default to current day, if month is december, else 1

        self.language = self.language
            .or(optional_parameters.language);
    }
}
