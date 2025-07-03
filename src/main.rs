use anyhow::Result;
use clap::Parser;
use handlebars::Handlebars;
use std::process::Command;

mod args;
mod config;
use args::{Args, Mode};
use config::Config;

fn main() -> Result<()> {
    let args = Args::parse();

    if args.mode == Mode::Url {
        let handlebars = Handlebars::new();
        println!(
            "{}",
            handlebars.render_template("https://adventofcode.com/{{year}}/day/{{day}}", &args)?
        );
    } else {
        let config = Config::load(&args)?;

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
