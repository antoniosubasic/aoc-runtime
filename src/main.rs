use anyhow::Result;
use clap::Parser;

mod args;
mod config;
use args::{Args, Mode};
use config::Config;

fn main() -> Result<()> {
    let args = Args::parse();
    let config = Config::load(&args)?;

    match args.mode {
        Mode::Config => {}
        Mode::Run => {}
    }

    Ok(())
}
