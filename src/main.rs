use anyhow::{Result, anyhow};
use aoc_api::Session;
use clap::Parser;
use colored::Colorize;
use std::{
    fs,
    process::{Command, Output},
};

mod args;
mod config;
use args::{Args, Mode};
use config::Config;

#[macro_export]
macro_rules! command {
    // program with no arguments
    ($program:expr) => {
        Command::new($program)
    };

    // program with arguments
    ($program:expr, $($arg:expr),+ $(,)?) => {
        {
            let mut cmd = Command::new($program);
            $(cmd.arg($arg);)*
            cmd
        }
    };
}

pub fn eval_command_output(output: &Output, silent: bool) -> Result<()> {
    match output.status.success() {
        true => {
            if !silent {
                println!("{}", String::from_utf8_lossy(&output.stdout));
            }
            Ok(())
        }
        false => Err(anyhow!(
            "failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let (mut config, optional_parameters) = Config::load()?;
    let mut args = Args::parse();

    args.build(optional_parameters);
    config.build(&args)?;

    // throw error if modes run, init, path, code are used without a language
    if matches!(args.mode, Mode::Run | Mode::Init | Mode::Path | Mode::Code)
        && args.language.is_none()
    {
        return Err(anyhow!("language is required for mode '{:?}'", args.mode));
    }

    // throw error if project doesn't exist for modes that require existence
    if matches!(args.mode, Mode::Run | Mode::Code) && !config.project_path.exists() {
        return Err(anyhow!(
            "project does not exist: {}",
            config.project_path.display()
        ));
    }

    let session = config
        .cookie
        .as_ref()
        .map(|cookie| Session::new(cookie.clone(), args.year.unwrap(), args.day.unwrap()));

    // check for input file and download if necessary
    if matches!(args.mode, Mode::Run | Mode::Init) {
        let parent_path = config
            .project_path
            .parent()
            .ok_or(anyhow!("project path does not have a parent directory"))?;
        let input_file = parent_path.join("input.txt");

        if !input_file.exists() {
            if let Some(session) = &session {
                fs::create_dir_all(parent_path)?;
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
        Mode::Run => {
            // run build (if exists for given language) command silently (meaning stdout is not printed)
            args.language
                .unwrap()
                .build_command(&config)
                .map(|mut cmd| eval_command_output(&cmd.output()?, true))
                .transpose()?;

            let run_output = args.language.unwrap().run_command(&config).output()?;
            eval_command_output(&run_output, true)?;

            let stdout = String::from_utf8_lossy(&run_output.stdout).to_string();

            // create session if cookie is provided
            if let Some(session) = &session {
                // count number of \n to determine number of parts to validate
                let new_lines: Vec<usize> = stdout
                    .chars()
                    .enumerate()
                    .filter_map(|(i, c)| if c == '\n' { Some(i) } else { None })
                    .collect();

                // newlines must be:
                // 1 = first part
                // 2 = first and second part
                if (1..=2).contains(&new_lines.len()) {
                    // split stdout into parts based on newlines
                    let (part1, part2) = match new_lines.len() {
                        1 => (stdout.trim_end(), None),
                        2 => {
                            let (part1, part2) = stdout.split_at(new_lines[0]);
                            (part1, Some(part2[1..].trim_end()))
                        }
                        _ => unreachable!(),
                    };

                    let part1_success = session
                        .submit_answer_explicit_error(1, part1)
                        .await
                        .map_err(|e| anyhow!("{e}"))?;

                    println!(
                        "{}",
                        if part1_success {
                            part1.green()
                        } else {
                            part1.red()
                        }
                    );

                    // continue to part 2 if it exists
                    if let Some(part2) = part2 {
                        let part2_success = session
                            .submit_answer_explicit_error(2, part2)
                            .await
                            .map_err(|e| anyhow!("{e}"))?;

                        println!(
                            "{}",
                            if part2_success {
                                part2.green()
                            } else {
                                part2.red()
                            }
                        );
                    }

                    // validation was successful
                    // exit successfully to prevent further output
                    return Ok(());
                }
            }

            // if no session is provided or newlines are not 1 or 2, just print the output
            println!("{}", String::from_utf8_lossy(&run_output.stdout));
        }
        Mode::Init => {
            // throw error if trying to initialize but project already exists
            if config.project_path.exists() {
                return Err(anyhow!(
                    "project already exists: {}",
                    config.project_path.display()
                ));
            } else {
                fs::create_dir_all(&config.project_path)
                    .map_err(|e| anyhow!("failed to create project directory: {}", e))?;
            }

            eval_command_output(
                &args.language.unwrap().init_command(&config).output()?,
                false,
            )?;
        }
        Mode::Path => {
            println!("{}", config.project_path.display());
        }
        Mode::Code => {
            command!("code", &config.project_path).spawn()?;
        }
        Mode::Url => {
            println!(
                "https://adventofcode.com/{}/day/{}",
                args.year.unwrap(),
                args.day.unwrap()
            );
        }
    }

    Ok(())
}
