//! The `aoc` command line tool.

use aoc_runtime::{
    aoc::{AocClient, cache::FileCache, input::InputStore, live::LiveClient},
    app::App,
    build::BuildStore,
    cli::Cli,
    config::Config,
    env::{Env, SystemClock},
    error::{self, Error},
    process::SystemRunner,
    report::{Reporter, TermConfirm, TermReporter},
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

    let plans = resolve::plan(cli, &config, &env.cwd, &SystemClock)?;

    let client = config
        .cookie
        .as_deref()
        .map(|cookie| LiveClient::new(cookie, &env.state_dir))
        .transpose()?;
    let cache = FileCache::new(&env.state_dir);
    let inputs = InputStore::new(&env.state_dir);
    let builds = BuildStore::new(&env.state_dir);
    let mut confirm = TermConfirm::new(cli.yes);

    let mut app = App {
        config: &config,
        runner: &SystemRunner,
        client: client.as_ref().map(|client| client as &dyn AocClient),
        cache: &cache,
        inputs: &inputs,
        builds: &builds,
        reporter: &mut reporter,
        confirm: &mut confirm,
    };

    app.execute_all(plans)
}
