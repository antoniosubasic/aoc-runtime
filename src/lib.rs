//! The engine behind the `aoc` command line tool.
//!
//! The binary is a thin shell around this library: it parses arguments, loads
//! configuration and hands the [`resolve::Plan`]s to [`app::App`], which
//! carries them out in order. Everything that
//! touches the outside world - the clock, child processes, the Advent of Code
//! API and the terminal - sits behind a trait, so the interesting logic is
//! exercised in tests without a network, a toolchain or a wall clock.
//!
//! ```no_run
//! use aoc_runtime::{
//!     app::App, build::BuildStore, cli::Cli, config::Config, env::{Env, SystemClock},
//!     aoc::{cache::FileCache, input::InputStore},
//!     process::SystemRunner, report::TermReporter, resolve,
//! };
//! use clap::Parser as _;
//!
//! let cli = Cli::parse();
//! let env = Env::capture(cli.config.as_deref())?;
//! let (config, _warnings) = Config::load(&env)?;
//! let plans = resolve::plan(&cli, &config, &env.cwd, &SystemClock)?;
//! let cache = FileCache::new(&env.state_dir);
//! let inputs = InputStore::new(&env.state_dir);
//! let builds = BuildStore::new(&env.state_dir);
//!
//! App {
//!     config: &config,
//!     runner: &SystemRunner,
//!     client: None,
//!     cache: &cache,
//!     inputs: &inputs,
//!     builds: &builds,
//!     reporter: &mut TermReporter,
//! }
//! .execute_all(plans)?;
//! # Ok::<(), aoc_runtime::error::Error>(())
//! ```

pub mod answer;
pub mod aoc;
pub mod app;
pub mod build;
pub mod cli;
pub mod config;
pub mod env;
pub mod error;
pub mod language;
pub mod process;
pub mod puzzle;
pub mod report;
pub mod resolve;
pub mod template;
