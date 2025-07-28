use anyhow::{Result, anyhow};
use aoc_api::Session;
use clap::Parser;
use handlebars::Handlebars;
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
        println!(
            "{}",
            Handlebars::new()
                .render_template("https://adventofcode.com/{{year}}/day/{{day}}", &args)?
        );
    } else {
        // make the language parameter required for all modes except "url"
        let language = args
            .language
            .ok_or_else(|| anyhow!("language is required for mode '{:?}'", args.mode))?;

        let config = Config::load(&args)?;

        if matches!(args.mode, Mode::Run | Mode::Code) && !config.project_path.exists() {
            return Err(anyhow!(
                "project does not exist: {}",
                config.project_path.display()
            ));
        }

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
                language
                    .build_command(&config)
                    .map(|mut cmd| eval_command_output(&cmd.output()?, true))
                    .transpose()?;

                eval_command_output(&language.run_command(&config).output()?, false)?;
            }
            Mode::Init => {
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
