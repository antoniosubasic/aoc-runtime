//! Supported solution languages and the commands that drive them.
//!
//! Everything a language needs - its CLI name, entry point and the commands
//! used to scaffold, build and run it - lives in a single match arm, so adding
//! a language is one variant and one arm.

use crate::process::{CommandSpec, ProcessError};
use clap::ValueEnum;
use std::{
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
};

/// The directory inside the configuration directory holding the base files,
/// each named after the language it starts.
pub const BASE_DIR_NAME: &str = "base";

/// A language a solution can be written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum)]
#[clap(rename_all = "lowercase")]
pub enum Language {
    /// Rust, scaffolded and driven through Cargo.
    Rust,
    /// C#, scaffolded and driven through the .NET SDK.
    CSharp,
    /// Java, compiled with `javac` and run with `java`.
    Java,
    /// Python, run directly by the interpreter.
    Python,
}

/// The commands used to scaffold, build and run a solution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageCommands {
    /// Scaffolding command, if the language needs more than an empty entry file.
    pub init: Option<CommandSpec>,
    /// Compilation command, if the language is compiled.
    pub build: Option<CommandSpec>,
    /// The command that executes the solution.
    pub run: CommandSpec,
    /// An alternative to try if [`LanguageCommands::run`]'s program is missing.
    pub run_fallback: Option<CommandSpec>,
}

impl Language {
    /// Every supported language.
    pub const ALL: &'static [Self] = &[Self::Rust, Self::CSharp, Self::Java, Self::Python];

    /// The language's canonical lowercase name, as used on the command line and
    /// in path templates.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::CSharp => "csharp",
            Self::Java => "java",
            Self::Python => "python",
        }
    }

    /// The optional starting-point file copied over a freshly scaffolded
    /// project, for example `~/.config/aoc/base/rust`.
    #[must_use]
    pub fn base_file(self, config_dir: &Path) -> PathBuf {
        config_dir.join(BASE_DIR_NAME).join(self.name())
    }

    /// The file inside a project that holds the solution.
    #[must_use]
    pub fn entry_file(self, project: &Path) -> PathBuf {
        match self {
            Self::Rust => project.join("src").join("main.rs"),
            Self::CSharp => project.join("Program.cs"),
            Self::Java => project.join("Main.java"),
            Self::Python => project.join("main.py"),
        }
    }

    /// Builds every command needed to work with a project in this language.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError::NoDirectoryName`] if a command needs the
    /// project's directory name but `project` has none, such as a filesystem
    /// root or a path ending in `..`.
    pub fn commands(self, project: &Path) -> Result<LanguageCommands, ProcessError> {
        let commands = match self {
            Self::Rust => {
                let manifest = project.join("Cargo.toml");
                LanguageCommands {
                    init: Some(
                        CommandSpec::new("cargo")
                            .args(["init", "--bin"])
                            .arg(project),
                    ),
                    build: Some(
                        CommandSpec::new("cargo")
                            .args(["build", "--release", "--manifest-path"])
                            .arg(&manifest),
                    ),
                    run: CommandSpec::new("cargo")
                        .args(["run", "--release", "--quiet", "--manifest-path"])
                        .arg(&manifest),
                    run_fallback: None,
                }
            }
            Self::CSharp => {
                let name = project
                    .file_name()
                    .ok_or_else(|| ProcessError::NoDirectoryName {
                        path: project.to_path_buf(),
                    })?;

                LanguageCommands {
                    init: Some(
                        CommandSpec::new("dotnet")
                            .args(["new", "console", "--name"])
                            .arg(name)
                            .arg("--output")
                            .arg(project),
                    ),
                    build: Some(
                        CommandSpec::new("dotnet")
                            .args(["build", "-c", "Release", "--nologo", "-v", "q"])
                            .arg(project),
                    ),
                    run: CommandSpec::new("dotnet")
                        .args(["run", "-c", "Release", "--no-build", "--project"])
                        .arg(project),
                    run_fallback: None,
                }
            }
            Self::Java => LanguageCommands {
                init: None,
                build: Some(CommandSpec::new("javac").arg(self.entry_file(project))),
                run: CommandSpec::new("java").arg("-cp").arg(project).arg("Main"),
                run_fallback: None,
            },
            Self::Python => LanguageCommands {
                init: None,
                build: None,
                run: CommandSpec::new("python3").arg(self.entry_file(project)),
                run_fallback: Some(CommandSpec::new("python").arg(self.entry_file(project))),
            },
        };

        Ok(commands.with_working_dir(project))
    }
}

impl LanguageCommands {
    fn with_working_dir(self, project: &Path) -> Self {
        Self {
            init: self.init.map(|spec| spec.current_dir(project)),
            build: self.build.map(|spec| spec.current_dir(project)),
            run: self.run.current_dir(project),
            run_fallback: self.run_fallback.map(|spec| spec.current_dir(project)),
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The error returned when a string does not name a supported language.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown language `{name}` (expected one of: rust, csharp, java, python)")]
pub struct UnknownLanguage {
    /// The unrecognised name.
    pub name: String,
}

impl FromStr for Language {
    type Err = UnknownLanguage;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|language| language.name() == value)
            .ok_or_else(|| UnknownLanguage {
                name: value.to_owned(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    fn project() -> PathBuf {
        PathBuf::from("/aoc/2024/day07/rust")
    }

    fn args_of(spec: &CommandSpec) -> Vec<String> {
        spec.arguments()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn every_variant_has_commands_and_a_unique_name() {
        let mut names: Vec<_> = Language::ALL.iter().map(|l| l.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Language::ALL.len());

        for language in Language::ALL {
            let commands = language
                .commands(&project())
                .expect("commands should build");
            assert_eq!(commands.run.working_dir(), Some(project().as_path()));
        }
    }

    #[test]
    fn names_match_the_command_line_values_clap_accepts() {
        for language in Language::ALL {
            let value = language
                .to_possible_value()
                .expect("every variant is selectable");
            assert_eq!(value.get_name(), language.name());
        }
    }

    #[test]
    fn parses_from_its_own_name() {
        for language in Language::ALL {
            assert_eq!(language.name().parse(), Ok(*language));
        }
        assert!("Rust".parse::<Language>().is_err());
        assert!("c-sharp".parse::<Language>().is_err());
    }

    #[test]
    fn rust_runs_the_optimized_build() {
        let commands = Language::Rust
            .commands(&project())
            .expect("commands should build");

        assert_eq!(commands.run.program(), "cargo");
        let args = args_of(&commands.run);
        assert!(args.contains(&"run".to_owned()), "{args:?}");
        assert!(args.contains(&"--release".to_owned()), "{args:?}");
        let manifest = project().join("Cargo.toml");
        assert!(
            args.contains(&manifest.to_string_lossy().into_owned()),
            "{args:?}"
        );
    }

    #[test]
    fn csharp_names_the_project_after_its_directory() {
        let commands = Language::CSharp
            .commands(Path::new("/aoc/2024/day07/csharp"))
            .expect("commands should build");
        let init = commands.init.expect("csharp scaffolds with dotnet new");

        assert_eq!(
            args_of(&init),
            [
                "new",
                "console",
                "--name",
                "csharp",
                "--output",
                "/aoc/2024/day07/csharp"
            ]
        );
    }

    #[test]
    fn csharp_run_reuses_the_build_output() {
        let commands = Language::CSharp
            .commands(&project())
            .expect("commands should build");

        assert!(args_of(&commands.run).contains(&"--no-build".to_owned()));
        assert!(
            args_of(&commands.build.expect("csharp is compiled")).contains(&"Release".to_owned())
        );
    }

    #[test]
    fn a_project_path_without_a_directory_name_is_an_error() {
        let error = Language::CSharp
            .commands(Path::new("/"))
            .expect_err("root has no directory name");

        assert!(
            matches!(error, ProcessError::NoDirectoryName { .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn python_is_interpreted_and_falls_back_to_python2_naming() {
        let commands = Language::Python
            .commands(Path::new("/aoc/2024/day07/python"))
            .expect("commands should build");

        assert!(commands.build.is_none());
        assert!(commands.init.is_none());
        assert_eq!(commands.run.program(), "python3");
        assert_eq!(
            commands.run_fallback.as_ref().map(CommandSpec::program),
            Some(OsStr::new("python"))
        );
    }

    #[test]
    fn java_compiles_and_runs_the_main_class() {
        let path = Path::new("/aoc/2024/day07/java");
        let commands = Language::Java
            .commands(path)
            .expect("commands should build");

        assert!(commands.init.is_none());
        assert_eq!(
            args_of(&commands.build.expect("java is compiled")),
            [path.join("Main.java").to_string_lossy().into_owned()]
        );
        assert_eq!(
            args_of(&commands.run),
            ["-cp", "/aoc/2024/day07/java", "Main"]
        );
    }

    #[test]
    fn entry_and_base_files_follow_language_conventions() {
        let config = Path::new("/home/u/.config/aoc");
        let project = Path::new("/aoc/2024/day07/x");

        let expected = [
            (
                Language::Rust,
                "/aoc/2024/day07/x/src/main.rs",
                "/home/u/.config/aoc/base/rust",
            ),
            (
                Language::CSharp,
                "/aoc/2024/day07/x/Program.cs",
                "/home/u/.config/aoc/base/csharp",
            ),
            (
                Language::Java,
                "/aoc/2024/day07/x/Main.java",
                "/home/u/.config/aoc/base/java",
            ),
            (
                Language::Python,
                "/aoc/2024/day07/x/main.py",
                "/home/u/.config/aoc/base/python",
            ),
        ];

        for (language, entry, base) in expected {
            assert_eq!(language.entry_file(project), Path::new(entry));
            assert_eq!(language.base_file(config), Path::new(base));
        }
    }
}
