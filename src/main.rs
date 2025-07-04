use anyhow::{Result, anyhow};
use aoc_api::Session;
use clap::Parser;
use handlebars::Handlebars;
use std::{fs, process::Command};

mod args;
mod config;
use args::{Args, Mode};
use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.mode == Mode::Url {
        let handlebars = Handlebars::new();
        println!(
            "{}",
            handlebars.render_template("https://adventofcode.com/{{year}}/day/{{day}}", &args)?
        );
    } else {
        // make the language parameter required for all modes except "url"
        if args.language.is_none() {
            return Err(anyhow!("language is required for mode '{:?}'", args.mode));
        }

        let config = Config::load(&args)?;

        if matches!(args.mode, Mode::Run | Mode::Code) && !config.project_path.exists() {
            return Err(anyhow!(
                "project does not exist: {}",
                config.project_path.display()
            ));
        }

        if matches!(args.mode, Mode::Run | Mode::Init) {
            let input_file = config
                .project_path
                .parent()
                .ok_or(anyhow!("project path does not have a parent directory"))?
                .join("input.txt");

            if !input_file.exists() {
                if let Some(session) = config
                    .cookie
                    .as_ref()
                    .map(|cookie| Session::new(cookie.clone(), args.year, args.day))
                {
                    fs::write(
                        &input_file,
                        session
                            .get_input_text()
                            .await
                            .map_err(|e| anyhow!("{}", e))?,
                    )?;
                }
            }
        }

        match args.mode {
            Mode::Run => {}
            Mode::Init => {}
            Mode::Path => {
                println!("{}", config.project_path.display());
            }
            Mode::Code => {
                Command::new("code").arg(&config.project_path).spawn()?;
            }
            Mode::Url => unreachable!(),
        }
    }

    Ok(())
}
