//! Supported solution languages and the commands that drive them.
//!
//! Everything a language needs - its CLI name, entry point and the commands
//! used to scaffold, build and run it - lives in a single match arm, so adding
//! a language is one variant and one arm.
//!
//! A compiled language builds optimized once and is then invoked directly, with
//! no build tool left in the loop, and it builds into the state directory so the
//! project holds sources and nothing else. Only a language whose build tool
//! cannot hand over a runnable artifact keeps running through that tool.

use crate::{
    build,
    process::{CommandSpec, ProcessError},
};
use clap::ValueEnum;
use std::{
    ffi::{OsStr, OsString},
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
    /// Rust, scaffolded by Cargo and built to an optimized binary.
    Rust,
    /// C#, scaffolded by the .NET SDK and built to a native launcher.
    CSharp,
    /// Java, compiled with `javac` and run with `java`.
    Java,
    /// Python, run directly by the interpreter.
    Python,
    /// JavaScript, run directly by Node.
    JavaScript,
    /// Go, scaffolded as a module and built to a binary.
    Go,
    /// C, compiled to a binary by the system compiler.
    C,
    /// C++, compiled to a binary by the system compiler.
    Cpp,
    /// Ruby, run directly by the interpreter.
    Ruby,
    /// Bash, run directly by the shell.
    Bash,
}

/// Where a solution's sources and its build output live.
///
/// The two directories are far apart - one in the solutions tree, one in the
/// state directory - and both are needed to describe a build, so they travel
/// together rather than as two paths a caller could pass the wrong way round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout<'a> {
    /// The project directory, holding the solution's sources.
    pub project: &'a Path,
    /// The directory this puzzle and language build into.
    pub artifacts: &'a Path,
}

/// The commands used to build and run a solution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageCommands {
    /// Compilation command, if the language is compiled.
    pub build: Option<CommandSpec>,
    /// The command that executes the solution.
    pub run: CommandSpec,
    /// An alternative to try if [`LanguageCommands::run`]'s program is missing.
    pub run_fallback: Option<CommandSpec>,
}

impl Language {
    /// Every supported language.
    pub const ALL: &'static [Self] = &[
        Self::Rust,
        Self::CSharp,
        Self::Java,
        Self::Python,
        Self::JavaScript,
        Self::Go,
        Self::C,
        Self::Cpp,
        Self::Ruby,
        Self::Bash,
    ];

    /// The language's canonical lowercase name, as used on the command line and
    /// in path templates.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::CSharp => "csharp",
            Self::Java => "java",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::Go => "go",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Ruby => "ruby",
            Self::Bash => "bash",
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
        let name = match self {
            Self::Rust => return project.join("src").join("main.rs"),
            Self::CSharp => "Program.cs",
            Self::Java => "Main.java",
            Self::Python => "main.py",
            Self::JavaScript => "main.js",
            Self::Go => "main.go",
            Self::C => "main.c",
            Self::Cpp => "main.cpp",
            Self::Ruby => "main.rb",
            Self::Bash => "main.sh",
        };

        project.join(name)
    }

    /// The command that turns an empty directory into a project, for the
    /// languages that need more than an entry file.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError::NoDirectoryName`] if the command needs the
    /// project's directory name but `project` has none, such as a filesystem
    /// root or a path ending in `..`.
    pub fn scaffold(self, project: &Path) -> Result<Option<CommandSpec>, ProcessError> {
        let command = match self {
            Self::Rust => CommandSpec::new("cargo")
                .args(["init", "--bin"])
                .arg(project),
            // The project is restored by the first build, which puts its
            // intermediate output in the build directory; restoring here would
            // instead leave an `obj` directory behind in the project.
            Self::CSharp => CommandSpec::new("dotnet")
                .args(["new", "console", "--no-restore", "--name"])
                .arg(directory_name(project)?)
                .arg("--output")
                .arg(project),
            // `go` is a module path the toolchain reserves, and under the
            // obvious template the project directory is named after the
            // language - so the module is qualified rather than named bare.
            // A module path is always slash-separated, on every platform.
            Self::Go => {
                let mut module = OsString::from("aoc/");
                module.push(directory_name(project)?);

                CommandSpec::new("go").arg("mod").arg("init").arg(module)
            }
            Self::Java
            | Self::Python
            | Self::JavaScript
            | Self::C
            | Self::Cpp
            | Self::Ruby
            | Self::Bash => return Ok(None),
        };

        Ok(Some(command.current_dir(project)))
    }

    /// The directory a build needs created for it beforehand.
    ///
    /// A language whose build tool owns a directory inside the project creates
    /// that itself, and an interpreted language builds nothing at all, so both
    /// answer `None` - and neither leaves an empty directory in the state
    /// directory behind.
    #[must_use]
    pub fn build_directory(self, layout: Layout<'_>) -> Option<&Path> {
        match self {
            Self::Java | Self::Go | Self::C | Self::Cpp => Some(layout.artifacts),
            Self::Rust
            | Self::CSharp
            | Self::Python
            | Self::JavaScript
            | Self::Ruby
            | Self::Bash => None,
        }
    }

    /// Builds the commands that compile and execute a project in this language.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError::NoDirectoryName`] if a command needs the
    /// project's directory name but the project path has none, such as a
    /// filesystem root or a path ending in `..`.
    pub fn commands(self, layout: Layout<'_>) -> Result<LanguageCommands, ProcessError> {
        let Layout { project, artifacts } = layout;
        let binary = build::binary(artifacts);

        let commands = match self {
            Self::Rust => cargo_commands(project)?,
            Self::CSharp => dotnet_commands(project)?,
            // Java has no binary to hand over, so it keeps its two-step launch;
            // only the class files move out of the project.
            Self::Java => LanguageCommands {
                build: Some(
                    CommandSpec::new("javac")
                        .arg("-d")
                        .arg(artifacts)
                        .arg(self.entry_file(project)),
                ),
                run: CommandSpec::new("java")
                    .arg("-cp")
                    .arg(artifacts)
                    .arg("Main"),
                run_fallback: None,
            },
            Self::Go => LanguageCommands {
                build: Some(
                    CommandSpec::new("go")
                        .arg("build")
                        .arg("-o")
                        .arg(&binary)
                        .arg("."),
                ),
                run: CommandSpec::new(&binary),
                run_fallback: None,
            },
            // `cc` and `c++` are whichever compiler the system installed, which
            // saves guessing between gcc and clang.
            Self::C | Self::Cpp => {
                let compiler = if self == Self::C { "cc" } else { "c++" };

                LanguageCommands {
                    build: Some(
                        CommandSpec::new(compiler)
                            .arg("-O2")
                            .arg("-o")
                            .arg(&binary)
                            .arg(self.entry_file(project)),
                    ),
                    run: CommandSpec::new(&binary),
                    run_fallback: None,
                }
            }
            Self::Python => LanguageCommands {
                build: None,
                run: CommandSpec::new("python3").arg(self.entry_file(project)),
                run_fallback: Some(CommandSpec::new("python").arg(self.entry_file(project))),
            },
            Self::JavaScript => LanguageCommands {
                build: None,
                run: CommandSpec::new("node").arg(self.entry_file(project)),
                run_fallback: Some(CommandSpec::new("nodejs").arg(self.entry_file(project))),
            },
            Self::Ruby => LanguageCommands {
                build: None,
                run: CommandSpec::new("ruby").arg(self.entry_file(project)),
                run_fallback: None,
            },
            Self::Bash => LanguageCommands {
                build: None,
                run: CommandSpec::new("bash").arg(self.entry_file(project)),
                run_fallback: None,
            },
        };

        Ok(commands.with_working_dir(project))
    }

    /// Every language name, in the order [`Language::ALL`] lists them.
    fn names() -> String {
        Self::ALL
            .iter()
            .map(|language| language.name())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Rust builds with Cargo and then steps around it.
///
/// The build stays in `target/`, where Cargo, the editor tooling and the
/// `.gitignore` Cargo itself writes all expect it; only the run steps around
/// the tool. Cargo names the binary after the package, which `cargo init` takes
/// from the directory - a package renamed by hand is not worth guessing at, so
/// the fallback hands the run back to Cargo.
fn cargo_commands(project: &Path) -> Result<LanguageCommands, ProcessError> {
    let manifest = project.join("Cargo.toml");
    // Both the subcommand and its option, in that order.
    let cargo = |subcommand: &[&str]| {
        CommandSpec::new("cargo")
            .args(subcommand)
            .arg("--manifest-path")
            .arg(&manifest)
    };

    Ok(LanguageCommands {
        build: Some(cargo(&["build", "--release"])),
        run: CommandSpec::new(
            project
                .join("target")
                .join("release")
                .join(build::executable(directory_name(project)?)),
        ),
        run_fallback: Some(cargo(&["run", "--release", "--quiet"])),
    })
}

/// C# builds with the .NET SDK and then steps around it.
///
/// `dotnet build` writes a native launcher next to the assembly, so the
/// solution runs without the SDK driving it. The build stays under `bin/`, and
/// `obj/` stays where it lands, because the C# language server needs the
/// restore artifacts in the place it looks for them. `-o` is still needed: the
/// default output path carries the target framework, which is not knowable
/// without reading the project file.
///
/// The fallback rebuilds - the launcher it is standing in for is the only
/// thing that knows where the build went - so it is silenced: whatever it
/// writes to standard output is read as the solution's answers, and a single
/// line of build chatter turns two answers into raw output nobody submits.
fn dotnet_commands(project: &Path) -> Result<LanguageCommands, ProcessError> {
    let output = project.join("bin").join("Release");

    Ok(LanguageCommands {
        build: Some(
            CommandSpec::new("dotnet")
                .args(["build", "-c", "Release", "--nologo", "-v", "q", "-o"])
                .arg(&output)
                .arg(project),
        ),
        run: CommandSpec::new(output.join(build::executable(directory_name(project)?))),
        run_fallback: Some(
            CommandSpec::new("dotnet")
                .args(["run", "-c", "Release", "-v", "q", "--project"])
                .arg(project),
        ),
    })
}

/// The name of the directory a project sits in, which the languages scaffolded
/// by a build tool are named after.
fn directory_name(project: &Path) -> Result<&OsStr, ProcessError> {
    project
        .file_name()
        .ok_or_else(|| ProcessError::NoDirectoryName {
            path: project.to_path_buf(),
        })
}

impl LanguageCommands {
    /// Runs everything from the project directory, wherever the binary itself
    /// ended up: a solution reads its input from `../input.txt`.
    fn with_working_dir(self, project: &Path) -> Self {
        Self {
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
#[error("unknown language `{name}` (expected one of: {})", Language::names())]
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

    const ARTIFACTS: &str = "/state/builds/2024-07/c";

    /// A path spelled the way the code under test spells it: it joins the
    /// components it adds, so the separator between them is the platform's.
    fn joined(base: impl AsRef<Path>, components: &str) -> String {
        components
            .split('/')
            .fold(base.as_ref().to_path_buf(), |path, component| {
                path.join(component)
            })
            .to_string_lossy()
            .into_owned()
    }

    /// A built binary's path, spelled the way the platform spells both a
    /// separator and an executable.
    fn built(directory: impl AsRef<Path>, stem: &str) -> String {
        directory
            .as_ref()
            .join(build::executable(stem))
            .to_string_lossy()
            .into_owned()
    }

    fn project() -> PathBuf {
        PathBuf::from("/aoc/2024/day07/rust")
    }

    /// Where a language's build output lands, keyed like `BuildStore` keys it.
    fn artifacts(language: Language) -> PathBuf {
        PathBuf::from("/state/builds/2024-07").join(language.name())
    }

    fn layout<'a>(project: &'a Path, artifacts: &'a Path) -> Layout<'a> {
        Layout { project, artifacts }
    }

    fn commands_for(language: Language, project: &Path) -> LanguageCommands {
        language
            .commands(layout(project, &artifacts(language)))
            .expect("commands should build")
    }

    fn args_of(spec: &CommandSpec) -> Vec<String> {
        spec.arguments()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    fn paths_of(spec: &CommandSpec) -> Vec<String> {
        let mut paths = args_of(spec);
        paths.push(spec.program().to_string_lossy().into_owned());
        paths
    }

    #[test]
    fn every_variant_has_commands_and_a_unique_name() {
        let mut names: Vec<_> = Language::ALL.iter().map(|l| l.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Language::ALL.len());

        for language in Language::ALL {
            let project = project().with_file_name(language.name());
            let commands = commands_for(*language, &project);
            assert_eq!(commands.run.working_dir(), Some(project.as_path()));
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
    fn an_unknown_language_is_offered_every_name_there_is() {
        let error = "brainfuck"
            .parse::<Language>()
            .expect_err("brainfuck is not supported");
        let message = error.to_string();

        for language in Language::ALL {
            assert!(message.contains(language.name()), "{message}");
        }
    }

    #[test]
    fn a_solution_always_runs_from_its_project_directory() {
        // Solutions read `../input.txt`, so where the binary sits is beside the
        // point - the working directory has to stay put.
        for language in Language::ALL {
            let project = project().with_file_name(language.name());
            let commands = commands_for(*language, &project);

            for spec in [Some(&commands.run), commands.run_fallback.as_ref()]
                .into_iter()
                .flatten()
            {
                assert_eq!(
                    spec.working_dir(),
                    Some(project.as_path()),
                    "{} runs elsewhere",
                    language.name()
                );
            }
        }
    }

    #[test]
    fn a_manually_compiled_language_writes_nothing_into_the_project() {
        for language in Language::ALL {
            let project = project().with_file_name(language.name());
            let artifacts = artifacts(*language);
            let commands = commands_for(*language, &project);

            // Cargo and the .NET SDK build inside the project on purpose; the
            // rule is for the languages aimed at the state directory.
            let (Some(build), Some(_)) = (
                &commands.build,
                language.build_directory(layout(&project, &artifacts)),
            ) else {
                continue;
            };

            // The only paths a build may name inside the project are the ones
            // it reads: the sources and the manifest describing them.
            let inputs: Vec<String> = [
                language.entry_file(&project),
                project.join("Cargo.toml"),
                project.clone(),
            ]
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();

            for path in paths_of(build) {
                assert!(
                    !path.starts_with(&*project.to_string_lossy()) || inputs.contains(&path),
                    "{} writes {path} into the project",
                    language.name()
                );
            }
        }
    }

    #[test]
    fn rust_runs_the_compiled_binary_instead_of_cargo() {
        let commands = commands_for(Language::Rust, &project());

        assert_eq!(
            commands.run.program(),
            OsStr::new(&built(joined(project(), "target/release"), "rust"))
        );
        assert!(args_of(&commands.run).is_empty());
    }

    #[test]
    fn rust_builds_optimized_into_cargos_own_target_directory() {
        let commands = commands_for(Language::Rust, &project());
        let build = commands.build.expect("rust is compiled");

        // Asserted in full: the option belongs to the subcommand, so a spec
        // that merely mentions it in some other order is not a valid cargo
        // invocation.
        assert_eq!(build.program(), OsStr::new("cargo"));
        assert_eq!(
            args_of(&build),
            [
                "build",
                "--release",
                "--manifest-path",
                &joined(project(), "Cargo.toml")
            ]
        );
    }

    #[test]
    fn a_renamed_rust_package_falls_back_to_cargo() {
        // The binary is named after the package, which `cargo init` takes from
        // the directory; a package renamed by hand is not worth guessing at.
        let commands = commands_for(Language::Rust, &project());
        let fallback = commands.run_fallback.expect("rust falls back to cargo");

        assert_eq!(fallback.program(), OsStr::new("cargo"));
        assert_eq!(
            args_of(&fallback),
            [
                "run",
                "--release",
                "--quiet",
                "--manifest-path",
                &joined(project(), "Cargo.toml")
            ]
        );
    }

    #[test]
    fn csharp_names_the_project_after_its_directory() {
        let scaffold = Language::CSharp
            .scaffold(Path::new("/aoc/2024/day07/csharp"))
            .expect("commands should build")
            .expect("csharp scaffolds with dotnet new");

        assert_eq!(
            args_of(&scaffold),
            [
                "new",
                "console",
                "--no-restore",
                "--name",
                "csharp",
                "--output",
                "/aoc/2024/day07/csharp"
            ]
        );
    }

    #[test]
    fn csharp_runs_the_launcher_the_build_produced() {
        let project = PathBuf::from("/aoc/2024/day07/csharp");
        let commands = commands_for(Language::CSharp, &project);
        let build = commands.build.expect("csharp is compiled");

        let output = joined(&project, "bin/Release");

        assert_eq!(
            commands.run.program(),
            OsStr::new(&built(&output, "csharp"))
        );
        assert_eq!(
            args_of(&build),
            [
                "build",
                "-c",
                "Release",
                "--nologo",
                "-v",
                "q",
                "-o",
                &output,
                &*project.to_string_lossy()
            ]
        );
    }

    #[test]
    fn the_csharp_fallback_prints_nothing_of_its_own() {
        // Its standard output is read as the solution's answers, so a single
        // line of build chatter would turn them into raw output nobody
        // submits.
        let project = PathBuf::from("/aoc/2024/day07/csharp");
        let commands = commands_for(Language::CSharp, &project);
        let fallback = commands.run_fallback.expect("csharp falls back to the sdk");

        assert_eq!(fallback.program(), OsStr::new("dotnet"));
        assert_eq!(
            args_of(&fallback),
            [
                "run",
                "-c",
                "Release",
                "-v",
                "q",
                "--project",
                "/aoc/2024/day07/csharp"
            ]
        );
    }

    #[test]
    fn a_project_path_without_a_directory_name_is_an_error() {
        let error = Language::CSharp
            .scaffold(Path::new("/"))
            .expect_err("root has no directory name");

        assert!(
            matches!(error, ProcessError::NoDirectoryName { .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn python_is_interpreted_and_falls_back_to_python2_naming() {
        let project = PathBuf::from("/aoc/2024/day07/python");
        let commands = commands_for(Language::Python, &project);

        assert!(commands.build.is_none());
        assert!(
            Language::Python
                .scaffold(&project)
                .expect("scaffolding should build")
                .is_none()
        );
        assert_eq!(commands.run.program(), OsStr::new("python3"));
        assert_eq!(
            commands.run_fallback.as_ref().map(CommandSpec::program),
            Some(OsStr::new("python"))
        );
    }

    #[test]
    fn javascript_falls_back_to_the_debian_binary_name() {
        let project = PathBuf::from("/aoc/2024/day07/javascript");
        let commands = commands_for(Language::JavaScript, &project);

        assert!(commands.build.is_none());
        assert_eq!(commands.run.program(), OsStr::new("node"));
        assert_eq!(args_of(&commands.run), [joined(&project, "main.js")]);
        assert_eq!(
            commands.run_fallback.as_ref().map(CommandSpec::program),
            Some(OsStr::new("nodejs"))
        );
    }

    #[test]
    fn java_compiles_and_runs_the_main_class_out_of_tree() {
        let project = PathBuf::from("/aoc/2024/day07/java");
        let commands = commands_for(Language::Java, &project);

        let classes = artifacts(Language::Java).to_string_lossy().into_owned();

        assert_eq!(
            args_of(&commands.build.expect("java is compiled")),
            ["-d", &classes, &joined(&project, "Main.java")]
        );
        assert_eq!(commands.run.program(), OsStr::new("java"));
        assert_eq!(args_of(&commands.run), ["-cp", &classes, "Main"]);
    }

    #[test]
    fn go_scaffolds_a_module_named_after_its_directory() {
        let project = PathBuf::from("/aoc/2024/day07/go");
        let scaffold = Language::Go
            .scaffold(&project)
            .expect("scaffolding should build")
            .expect("go scaffolds a module");

        assert_eq!(scaffold.program(), OsStr::new("go"));
        assert_eq!(args_of(&scaffold), ["mod", "init", "aoc/go"]);
        assert_eq!(scaffold.working_dir(), Some(project.as_path()));
    }

    #[test]
    fn a_go_module_is_never_named_after_a_reserved_path() {
        // `go mod init go` is rejected outright, and a project directory named
        // after its language is exactly what the obvious template produces.
        for directory in ["go", "cmd"] {
            let project = project().with_file_name(directory);
            let scaffold = Language::Go
                .scaffold(&project)
                .expect("scaffolding should build")
                .expect("go scaffolds a module");

            assert_eq!(
                args_of(&scaffold),
                ["mod", "init", &format!("aoc/{directory}")]
            );
        }
    }

    #[test]
    fn go_builds_the_module_to_the_state_directory() {
        let project = PathBuf::from("/aoc/2024/day07/go");
        let commands = commands_for(Language::Go, &project);

        let binary = built(artifacts(Language::Go), "bin");

        assert_eq!(
            args_of(&commands.build.expect("go is compiled")),
            ["build", "-o", &binary, "."]
        );
        assert_eq!(commands.run.program(), OsStr::new(&binary));
    }

    #[test]
    fn c_and_cpp_optimize_through_the_system_compiler() {
        let expected = [
            (Language::C, "cc", "main.c"),
            (Language::Cpp, "c++", "main.cpp"),
        ];

        for (language, compiler, entry) in expected {
            let project = project().with_file_name(language.name());
            let commands = commands_for(language, &project);
            let build = commands.build.expect("compiled");
            let binary = built(artifacts(language), "bin");

            assert_eq!(build.program(), OsStr::new(compiler));
            assert_eq!(
                args_of(&build),
                [
                    "-O2",
                    "-o",
                    &binary,
                    &*project.join(entry).to_string_lossy()
                ]
            );
            assert_eq!(commands.run.program(), OsStr::new(&binary));
            assert!(commands.run_fallback.is_none());
        }
    }

    #[test]
    fn ruby_and_bash_are_handed_straight_to_their_interpreter() {
        let expected = [
            (Language::Ruby, "ruby", "main.rb"),
            (Language::Bash, "bash", "main.sh"),
        ];

        for (language, program, entry) in expected {
            let project = project().with_file_name(language.name());
            let commands = commands_for(language, &project);

            assert!(commands.build.is_none());
            assert_eq!(commands.run.program(), OsStr::new(program));
            assert_eq!(
                args_of(&commands.run),
                [project.join(entry).to_string_lossy().into_owned()]
            );
        }
    }

    #[test]
    fn entry_and_base_files_follow_language_conventions() {
        let config = Path::new("/home/u/.config/aoc");
        let project = Path::new("/aoc/2024/day07/x");

        let expected = [
            (Language::Rust, "src/main.rs"),
            (Language::CSharp, "Program.cs"),
            (Language::Java, "Main.java"),
            (Language::Python, "main.py"),
            (Language::JavaScript, "main.js"),
            (Language::Go, "main.go"),
            (Language::C, "main.c"),
            (Language::Cpp, "main.cpp"),
            (Language::Ruby, "main.rb"),
            (Language::Bash, "main.sh"),
        ];

        assert_eq!(expected.len(), Language::ALL.len());

        for (language, entry) in expected {
            assert_eq!(language.entry_file(project), project.join(entry));
            assert_eq!(
                language.base_file(config),
                config.join("base").join(language.name())
            );
        }
    }

    #[test]
    fn the_build_directory_is_the_one_it_was_handed() {
        let project = project().with_file_name("c");
        let artifacts = Path::new(ARTIFACTS);
        let commands = Language::C
            .commands(layout(&project, artifacts))
            .expect("commands should build");

        assert_eq!(
            Language::C.build_directory(layout(&project, artifacts)),
            Some(artifacts)
        );
        assert!(
            args_of(&commands.build.expect("c is compiled")).contains(&built(ARTIFACTS, "bin"))
        );
    }

    #[test]
    fn only_a_manually_compiled_language_asks_for_a_state_directory() {
        // Cargo and the .NET SDK make and own `target/` and `bin/`; creating a
        // state directory for them would only leave an empty one behind, and an
        // interpreted language builds nothing at all.
        let artifacts = Path::new(ARTIFACTS);

        for language in Language::ALL {
            let project = project().with_file_name(language.name());
            let wanted = language
                .build_directory(layout(&project, artifacts))
                .is_some();
            let expected = matches!(
                language,
                Language::Java | Language::Go | Language::C | Language::Cpp
            );

            assert_eq!(wanted, expected, "{}", language.name());
            if wanted {
                assert!(
                    commands_for(*language, &project).build.is_some(),
                    "{} asks for a directory but never builds",
                    language.name()
                );
            }
        }
    }
}
