use anyhow::{Context, Result, anyhow};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{env, fs, ops::Range, path::PathBuf};
use strum::IntoEnumIterator;

use crate::args::{Args, Language};

pub struct OptionalParameters {
    pub year: Option<u16>,
    pub day: Option<u8>,
    pub language: Option<Language>,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    template_path: String,
    pub cookie: Option<String>,
    #[serde(skip)]
    pub project_path: PathBuf,
}

impl Config {
    // helper function
    fn build_param_regex(param: &str, paddable: bool) -> Regex {
        Regex::new(&format!(
            r"\{{\{{\s*{}{}\s*\}}\}}",
            if paddable { r"(pad\s+)?" } else { "" },
            param
        ))
        .unwrap()
    }

    pub fn load() -> Result<(Self, OptionalParameters)> {
        let home = dirs::home_dir().context("could not determine home directory")?;

        let config_path = home.join(".config").join("aoc").join("config.yaml");
        let config_content = fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read config file '{}'", config_path.display()))?;

        let mut config: Config = serde_yml::from_str(&config_content)
            .with_context(|| format!("failed to parse config file '{}'", config_path.display()))?;

        if let Some(stripped) = config.template_path.strip_prefix("~/") {
            config.template_path = home.join(stripped).to_string_lossy().to_string();
        }

        let mut optional_params = OptionalParameters {
            year: None,
            day: None,
            language: None,
        };

        // algorithm to escape template path and insert regex patterns for parameter extraction
        // then use pattern to extract parameters from the current working directory
        {
            let mut pattern = String::new();

            // escaping the template path for later regex use
            {
                // get the location ranges of regex patterns within the template path
                let mut regex_patterns_locations: Vec<Range<usize>> =
                    [("year", false), ("day", true), ("language", false)]
                        .into_iter()
                        .flat_map(|(name, paddable)| {
                            let re = Config::build_param_regex(name, paddable);
                            re.find_iter(&config.template_path)
                                .map(|m| m.start()..m.end())
                                .collect::<Vec<Range<usize>>>()
                        })
                        .collect();

                regex_patterns_locations.sort_by_key(|range| range.start);

                let mut previous = 0;

                // build the pattern by
                // 1. escaping the template path substrings which are not contained within the regex_patterns_locations ranges
                // 2. inserting the regex patterns (unescaped) for each range
                for range in &regex_patterns_locations {
                    if previous < range.start {
                        pattern
                            .push_str(&regex::escape(&config.template_path[previous..range.start]));
                    }

                    pattern.push_str(&config.template_path[range.clone()]);
                    previous = range.end;
                }

                if previous < config.template_path.len() {
                    pattern.push_str(&regex::escape(&config.template_path[previous..]));
                }
            }

            // insert the actual regex patterns for parameter extraction
            {
                let replacements = [
                    ("{{year}}", r"(?P<year>\d{4})"),
                    ("{{day}}", r"(?P<day>[1-9]|1[0-9]|2[0-5])"),
                    ("{{pad day}}", r"(?P<padday>0[1-9]|1[0-9]|2[0-5])"),
                    (
                        "{{language}}",
                        &format!(
                            "(?P<language>{})",
                            Language::iter()
                                .map(|l| l.to_string())
                                .collect::<Vec<String>>()
                                .join("|")
                        ),
                    ),
                ];

                let mut end_pattern = String::new();

                // insert optional group after each pattern that lasts until the end of the string
                // to allow for partial parameter extraction
                for i in 0..replacements.len() {
                    if pattern.find(replacements[i].0).is_some() {
                        pattern = pattern.replace(
                            replacements[i].0,
                            &format!(
                                "{}{}",
                                replacements[i].1,
                                if i < replacements.len() - 1 { "(" } else { "" }
                            ),
                        );
                        if i < replacements.len() - 1 {
                            end_pattern.push_str(")?");
                        }
                    }
                }

                pattern.push_str(&end_pattern);
            }

            // capture the parameters from the current working directory
            // and store them in the optional_params (later being used to override default arguments)
            if let Some(captures) =
                Regex::new(&pattern)?.captures(&env::current_dir()?.to_string_lossy().into_owned())
            {
                optional_params.year = captures
                    .name("year")
                    .map(|m| m.as_str().parse().ok())
                    .flatten();

                optional_params.day = captures
                    .name("day")
                    .or(captures.name("padday"))
                    .map(|m| m.as_str().parse().ok())
                    .flatten();

                optional_params.language = captures
                    .name("language")
                    .map(|m| m.as_str().parse().ok())
                    .flatten();
            }
        }

        Ok((config, optional_params))
    }

    pub fn build(&mut self, args: &Args) -> Result<()> {
        let mut path = self.template_path.clone();

        for (name, value, paddable) in [
            ("year", args.year.map(|y| y.to_string()), false),
            ("day", args.day.map(|d| d.to_string()), true),
            (
                "language",
                args.language.map(|lang| lang.to_string()),
                false,
            ),
        ]
        .into_iter()
        {
            if let Some(value) = value {
                let re = Config::build_param_regex(name, paddable);
                let captures = re
                    .captures(&path)
                    .ok_or_else(|| anyhow!("failed to find '{}' in template path", name))?;

                let paddable = captures.get(1).is_some();

                path = re
                    .replace_all(
                        &path,
                        if paddable {
                            format!("{:0>2}", value)
                        } else {
                            value
                        },
                    )
                    .to_string();
            }
        }

        self.project_path = PathBuf::from(path);

        Ok(())
    }
}
