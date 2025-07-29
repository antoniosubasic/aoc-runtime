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
use args::{Args, Language, Mode};
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
    let args = Args::parse();

    if args.mode == Mode::Url {
        println!("https://adventofcode.com/{}/day/{}", args.year, args.day);
    } else {
        // make the language parameter required for all modes except "url"
        let language = args
            .language
            .ok_or_else(|| anyhow!("language is required for mode '{:?}'", args.mode))?;

        let config = Config::load(&args)?;

        // throw error if project doesn't exist for modes that require existence
        if matches!(args.mode, Mode::Run | Mode::Code) && !config.project_path.exists() {
            return Err(anyhow!(
                "project does not exist: {}",
                config.project_path.display()
            ));
        }

        // check for input file and download if necessary
        if matches!(args.mode, Mode::Run | Mode::Init) {
            let parent_path = config
                .project_path
                .parent()
                .ok_or(anyhow!("project path does not have a parent directory"))?;
            let input_file = parent_path.join("input.txt");

            if !input_file.exists() {
                if let Some(session) = config
                    .cookie
                    .as_ref()
                    .map(|cookie| Session::new(cookie.clone(), args.year, args.day))
                {
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
                language
                    .build_command(&config)
                    .map(|mut cmd| eval_command_output(&cmd.output()?, true))
                    .transpose()?;

                let run_output = language.run_command(&config).output()?;
                eval_command_output(&run_output, true)?;

                let stdout = String::from_utf8_lossy(&run_output.stdout).to_string();

                // create session if cookie is provided
                if let Some(session) = config
                    .cookie
                    .as_ref()
                    .map(|cookie| Session::new(cookie.clone(), args.year, args.day))
                {
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

                        let part1_response = session
                            .submit_answer(1, part1)
                            .await
                            .map_err(|e| anyhow!("{e}"))?;

                        if let Some(part1_success) = part1_response.success {
                            // print result of part 1 if api call was successful
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
                                let part2_response = session
                                    .submit_answer(2, part2)
                                    .await
                                    .map_err(|e| anyhow!("{e}"))?;

                                if let Some(part2_success) = part2_response.success {
                                    // print result of part 2 if api call was successful
                                    println!(
                                        "{}",
                                        if part2_success {
                                            part2.green()
                                        } else {
                                            part2.red()
                                        }
                                    );
                                } else {
                                    // throw error if part 2 submission failed
                                    return Err(anyhow!(
                                        "failed to submit answer for part 2: {}",
                                        part2_response
                                            .cooldown
                                            .unwrap_or_else(|| "unknown error".to_string())
                                    ));
                                }
                            }
                        } else {
                            // throw error if part 1 submission failed
                            return Err(anyhow!(
                                "failed to submit answer for part 1: {}",
                                part1_response
                                    .cooldown
                                    .unwrap_or_else(|| "unknown error".to_string())
                            ));
                        }

                        // validation was successful
                        // exit successfully to prevent further output
                        return Ok(());
                    };
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

                println!(
                    "Creating project at: {}",
                    config.project_path.display().to_string()
                );

                let init_output = match language {
                    Language::Rust => {
                        command!("cargo", "init", "--bin", &config.project_path)
                    }
                    Language::CSharp => {
                        command!(
                            "dotnet",
                            "new",
                            "console",
                            "--name",
                            &config.project_path.file_name().unwrap().to_str().unwrap(),
                            "--output",
                            &config.project_path
                        )
                    }
                    Language::Java => {
                        if !config.project_path.join("src").exists() {
                            fs::create_dir_all(&config.project_path.join("src"))?;
                        }
                        command!("touch", &config.project_path.join("src").join("Main.java"))
                    }
                    Language::Python => {
                        command!("touch", &config.project_path.join("main.py"))
                    }
                }
                .output()?;
                eval_command_output(&init_output, false)?;
            }
            Mode::Path => {
                println!("{}", config.project_path.display());
            }
            Mode::Code => {
                command!("code", &config.project_path).spawn()?;
            }
            Mode::Url => unreachable!(),
        }
    }

    Ok(())
}
