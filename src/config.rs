use anyhow::{Context, Result};
use handlebars::{Context as HbContext, Handlebars, Helper, HelperResult, Output, RenderContext};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

use crate::args::Args;

#[derive(Serialize, Deserialize)]
pub struct Config {
    template_path: String,
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

        // this code block builds the actual project path based on the templated path provided in the config file
        // for that, it uses the library 'handlebars' with a custom helper, for left padding numerical values (year, day, etc.) with 0s
        {
            let mut handlebars = Handlebars::new();
            handlebars.register_helper("pad", Box::new(pad_helper));

            config.project_path = PathBuf::from(
                handlebars
                    .render_template(&config.template_path, args)
                    .with_context(|| {
                        format!(
                            "failed inserting variables in template path '{}'",
                            config.template_path
                        )
                    })?,
            );

            if config.project_path.starts_with("~") {
                config.project_path = home.join(config.project_path.strip_prefix("~").unwrap());
            }
        }

        Ok(config)
    }
}

fn pad_helper(
    h: &Helper,
    _: &Handlebars,
    _: &HbContext,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let value = h.param(0).and_then(|v| v.value().as_u64()).ok_or_else(|| {
        handlebars::RenderErrorReason::Other("first parameter must be a number".into())
    })?;

    let width = h.param(1).and_then(|v| v.value().as_u64()).unwrap_or(2) as usize;

    let padded = format!("{:0width$}", value, width = width);
    out.write(&padded)?;
    Ok(())
}
