use anyhow::{Result, anyhow};
use aoc_api::Session;
use clap::Parser;
use handlebars::Handlebars;
use std::{fs, process::Command};

mod args;
mod config;
use args::{Args, Language, Mode};
use config::Config;

macro_rules! cmd {
    ($program:expr, $($arg:expr),*) => {
        Command::new($program)
            $(.arg($arg.to_string()))*
            .status()
    };
}

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

        let project_path_src = config.project_path.join("src");

        match args.mode {
            Mode::Run => {
                if let Some(build_output) = language
                    .build_command(&config)
                    .map(|mut cmd| cmd.output())
                    .transpose()?
                {
                    if !build_output.status.success() {
                        return Err(anyhow!(
                            "build failed: {}",
                            String::from_utf8_lossy(&build_output.stderr)
                        ));
                    }
                }

                let run_output = language.run_command(&config).output()?;
                match run_output.status.success() {
                    true => println!("{}", String::from_utf8_lossy(&run_output.stdout)),
                    false => {
                        return Err(anyhow!(
                            "run failed: {}",
                            String::from_utf8_lossy(&run_output.stderr)
                        ));
                    }
                }
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

                match language {
                    Language::Rust => {
                        cmd!("cargo", "init", "--bin", config.project_path.display())?;
                    }
                    Language::CSharp => {
                        cmd!(
                            "dotnet",
                            "new",
                            "console",
                            "--name",
                            config.project_path.file_name().unwrap().to_str().unwrap(),
                            "--output",
                            config.project_path.display()
                        )?;
                    }
                    Language::Java => {
                        cmd!("mkdir", "-p", project_path_src.display())?;
                        cmd!("touch", project_path_src.join("Main.java").display())?;
                    }
                    Language::Python => {
                        cmd!("touch", config.project_path.join("main.py").display())?;
                    }
                };
            }
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
