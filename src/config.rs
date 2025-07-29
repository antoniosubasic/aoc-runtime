use anyhow::{Context, Result, anyhow};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

use crate::args::Args;

#[derive(Serialize, Deserialize)]
pub struct Config {
    template_path: String,
    pub cookie: Option<String>,
    #[serde(skip)]
    pub project_path: PathBuf,
}

impl Config {
    pub fn load(args: &Args) -> Result<Self> {
        let home = dirs::home_dir().context("could not determine home directory")?;

        let config_path = home.join(".config").join("aoc").join("config.yaml");
        let config_content = fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read config file '{}'", config_path.display()))?;

        let mut config: Config = serde_yml::from_str(&config_content)
            .with_context(|| format!("failed to parse config file '{}'", config_path.display()))?;

        if let Ok(stripped) = config.project_path.strip_prefix("~") {
            config.project_path = home.join(stripped);
        }

        config.build_from_args(args)?;

        Ok(config)
    }

    fn build_regex(param: &str, paddable: bool) -> Regex {
        Regex::new(&format!(
            r"\{{\{{\s*{}{}\s*\}}\}}",
            if paddable { r"(pad\s+)?" } else { "" },
            param
        ))
        .unwrap()
    }

    pub fn build_from_args(&mut self, args: &Args) -> Result<()> {
        let mut path = self.template_path.clone();

        for (name, value, paddable) in args.iter() {
            let re = Config::build_regex(name, paddable);
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

        self.project_path = PathBuf::from(path);

        Ok(())
    }
}
