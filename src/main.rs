//! The `aoc` command line tool.

use aoc_runtime::{
    aoc::{AocClient, cache::FileCache, live::LiveClient},
    app::App,
    cli::Cli,
    config::Config,
    env::{Env, SystemClock},
    error::{self, Error},
    process::SystemRunner,
    report::{Reporter, TermReporter},
    resolve,
};
use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error::report(&error);
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), Error> {
    let env = Env::capture(cli.config.as_deref())?;
    let (config, warnings) = Config::load(&env)?;

    let mut reporter = TermReporter;
    for warning in &warnings {
        reporter.warn(warning);
    }

    let plan = resolve::plan(cli, &config, &env.cwd, &SystemClock)?;

    let client = config
        .cookie
        .as_deref()
        .map(|cookie| LiveClient::new(cookie, &env.state_dir))
        .transpose()?;
    let cache = FileCache::new(&env.state_dir);

    App {
        config: &config,
        runner: &SystemRunner,
        client: client.as_ref().map(|client| client as &dyn AocClient),
        cache: &cache,
        reporter: &mut reporter,
    }
    .execute(plan)
}
