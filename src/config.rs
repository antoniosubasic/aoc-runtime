use anyhow::{Context, Result, anyhow};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{env, fs, ops::Range, path::PathBuf};
use strum::IntoEnumIterator;

use crate::args::{Args, Language};

#[derive(Serialize, Deserialize)]
pub struct Config {
    template_path: String,
    pub cookie: Option<String>,
    #[serde(skip)]
    pub project_path: PathBuf,
}

impl Config {
    pub fn load() -> Result<Self> {
        let home = dirs::home_dir().context("could not determine home directory")?;

        let config_path = home.join(".config").join("aoc").join("config.yaml");
        let config_content = fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read config file '{}'", config_path.display()))?;

        let mut config: Config = serde_yml::from_str(&config_content)
            .with_context(|| format!("failed to parse config file '{}'", config_path.display()))?;

        if let Some(stripped) = config.template_path.strip_prefix("~/") {
            config.template_path = home.join(stripped).to_string_lossy().to_string();
        }

        Ok(config)
    }

    fn build_param_regex(param: &str, paddable: bool) -> Regex {
        Regex::new(&format!(
            r"\{{\{{\s*{}{}\s*\}}\}}",
            if paddable { r"(pad\s+)?" } else { "" },
            param
        ))
        .unwrap()
    }

    fn build_path_regex(&self) -> Result<Regex> {
        let mut param_locations: Vec<Range<usize>> = Args::iter_names()
            .flat_map(|(name, paddable)| {
                let re = Config::build_param_regex(name, paddable);
                re.find_iter(&self.template_path)
                    .map(|m| m.start()..m.end())
                    .collect::<Vec<Range<usize>>>()
            })
            .collect();

        param_locations.sort_by_key(|range| range.start);

        let mut pattern = String::new();
        let mut previous = 0;

        for range in &param_locations {
            if previous < range.start {
                pattern.push_str(&regex::escape(&self.template_path[previous..range.start]));
            }

            pattern.push_str(&self.template_path[range.clone()]);
            previous = range.end;
        }

        if previous < self.template_path.len() {
            pattern.push_str(&regex::escape(&self.template_path[previous..]));
        }

        let pattern = pattern
            .replace("{{year}}", r"(?P<year>\d{4})")
            .replace("{{day}}", r"(?P<day>[1-9]|1[0-9]|2[0-5])")
            .replace("{{pad day}}", r"(?P<padday>0[1-9]|1[0-9]|2[0-5])")
            .replace(
                "{{language}}",
                &format!(
                    r"(?P<language>{})",
                    Language::iter()
                        .map(|l| l.to_string())
                        .collect::<Vec<String>>()
                        .join("|")
                ),
            );

        Ok(Regex::new(&pattern)?)
    }

    pub fn build(&mut self, args: &mut Args) -> Result<()> {
        let working_directory = env::current_dir()?.to_string_lossy().into_owned();

        if let Some(captures) = self.build_path_regex()?.captures(&working_directory) {
            if captures.len() == 4
                && captures.name("year").is_some()
                && (captures.name("day").is_some() || captures.name("padday").is_some())
                && captures.name("language").is_some()
            {
                if args.year.is_none() {
                    args.year = captures
                        .name("year")
                        .map(|m| m.as_str().parse::<u16>().ok())
                        .flatten();
                }

                if args.day.is_none() {
                    args.day = captures
                        .name("day")
                        .or(captures.name("padday"))
                        .map(|m| m.as_str().parse::<u8>().ok())
                        .flatten();
                }

                if args.language.is_none() {
                    args.language = captures
                        .name("language")
                        .map(|m| m.as_str().parse::<Language>().ok())
                        .flatten();
                }
            }
        }

        args.validate()?;

        let mut path = self.template_path.clone();

        for (name, value, paddable) in args.iter() {
            let re = Config::build_param_regex(name, paddable);
            let captures = re
                .captures(&path)
                .ok_or_else(|| anyhow!("failed to find '{}' in template path", name))?;
            let paddable = captures.get(1).is_some();
            path = re
                .replace_all(
                    &path,
                    if paddable {
                        format!("{:0>2}", value.unwrap_or("".to_string()))
                    } else {
                        value.unwrap_or("".to_string())
                    },
                )
                .to_string();
        }

        self.project_path = PathBuf::from(path);

        Ok(())
    }
}
