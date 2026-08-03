//! Child process execution.
//!
//! Commands are described as data ([`CommandSpec`]) and executed through the
//! [`CommandRunner`] trait, so the per-language command tables can be asserted
//! in tests without ever spawning `cargo`, `dotnet` or `javac`.

use std::{
    ffi::{OsStr, OsString},
    fmt::Write as _,
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// A fully described child process invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    program: OsString,
    args: Vec<OsString>,
    current_dir: Option<PathBuf>,
}

impl CommandSpec {
    /// Creates a spec for `program` with no arguments.
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        Self {
            program: program.as_ref().to_os_string(),
            args: Vec::new(),
            current_dir: None,
        }
    }

    /// Appends a single argument.
    #[must_use]
    pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    /// Appends multiple arguments.
    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_os_string()));
        self
    }

    /// Sets the working directory the command runs in.
    #[must_use]
    pub fn current_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.current_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    /// The program to execute.
    #[must_use]
    pub fn program(&self) -> &OsStr {
        &self.program
    }

    /// The arguments passed to the program.
    #[must_use]
    pub fn arguments(&self) -> &[OsString] {
        &self.args
    }

    /// The working directory, if one was set.
    #[must_use]
    pub fn working_dir(&self) -> Option<&Path> {
        self.current_dir.as_deref()
    }

    /// Renders the invocation for diagnostics, quoting arguments with spaces.
    #[must_use]
    pub fn to_display_string(&self) -> String {
        let mut rendered = self.program.to_string_lossy().into_owned();
        for arg in &self.args {
            let arg = arg.to_string_lossy();
            if arg.contains(char::is_whitespace) {
                let _ = write!(rendered, " \"{arg}\"");
            } else {
                let _ = write!(rendered, " {arg}");
            }
        }
        rendered
    }

    fn to_command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args);
        if let Some(dir) = &self.current_dir {
            command.current_dir(dir);
        }
        command
    }
}

/// What to do with one of a child process's output streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    /// Capture the stream for inspection.
    Capture,
    /// Pass the stream through to this process's own.
    Inherit,
}

impl Stream {
    fn stdio(self) -> Stdio {
        match self {
            Self::Capture => Stdio::piped(),
            Self::Inherit => Stdio::inherit(),
        }
    }
}

/// How a child process's output streams are handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoPolicy {
    /// Handling for standard output.
    pub stdout: Stream,
    /// Handling for standard error.
    pub stderr: Stream,
}

impl IoPolicy {
    /// Capture everything - used for build steps, whose output is only
    /// interesting when they fail.
    pub const SILENT: Self = Self {
        stdout: Stream::Capture,
        stderr: Stream::Capture,
    };

    /// Capture standard output but let standard error through live - used for
    /// the solution itself, whose stdout carries the answers and whose stderr
    /// carries progress the user should see as it happens.
    pub const ANSWER: Self = Self {
        stdout: Stream::Capture,
        stderr: Stream::Inherit,
    };

    /// Pass both streams through - used for scaffolding commands.
    pub const INHERIT: Self = Self {
        stdout: Stream::Inherit,
        stderr: Stream::Inherit,
    };
}

/// The captured result of a finished child process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    /// The exit code, or `None` if the process was terminated by a signal.
    pub code: Option<i32>,
    /// Standard output, lossily decoded as UTF-8, empty if it was inherited.
    pub stdout: String,
    /// Standard error, lossily decoded as UTF-8, empty if it was inherited.
    pub stderr: String,
}

impl CommandOutput {
    /// Whether the process exited successfully.
    #[must_use]
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    fn status_description(&self) -> String {
        self.code.map_or_else(
            || "terminated by signal".to_owned(),
            |code| format!("exit code {code}"),
        )
    }
}

/// Executes [`CommandSpec`]s.
pub trait CommandRunner {
    /// Runs the command to completion.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError::NotFound`] or [`ProcessError::Spawn`] if the
    /// program could not be started.
    fn run(&self, spec: &CommandSpec, io: IoPolicy) -> Result<CommandOutput, ProcessError>;

    /// Starts the command without waiting for it to finish.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError::NotFound`] or [`ProcessError::Spawn`] if the
    /// program could not be started.
    fn spawn_detached(&self, spec: &CommandSpec) -> Result<(), ProcessError>;

    /// Runs the command and fails if it exits unsuccessfully.
    ///
    /// # Errors
    ///
    /// As [`CommandRunner::run`], plus [`ProcessError::Failed`] if the process
    /// exited with a non-success status.
    fn run_checked(&self, spec: &CommandSpec, io: IoPolicy) -> Result<CommandOutput, ProcessError> {
        let output = self.run(spec, io)?;
        if output.success() {
            Ok(output)
        } else {
            Err(ProcessError::Failed {
                command: spec.to_display_string(),
                status: output.status_description(),
                stderr: output.stderr.trim_end().to_owned(),
            })
        }
    }
}

/// Runs commands as real child processes.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, spec: &CommandSpec, io: IoPolicy) -> Result<CommandOutput, ProcessError> {
        let output = spec
            .to_command()
            .stdin(Stdio::null())
            .stdout(io.stdout.stdio())
            .stderr(io.stderr.stdio())
            .spawn()
            .map_err(|source| spawn_error(spec, source))?
            .wait_with_output()
            .map_err(|source| spawn_error(spec, source))?;

        Ok(CommandOutput {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    fn spawn_detached(&self, spec: &CommandSpec) -> Result<(), ProcessError> {
        spec.to_command()
            .spawn()
            .map(|_| ())
            .map_err(|source| spawn_error(spec, source))
    }
}

fn spawn_error(spec: &CommandSpec, source: io::Error) -> ProcessError {
    if source.kind() == io::ErrorKind::NotFound {
        ProcessError::NotFound {
            program: spec.program().to_string_lossy().into_owned(),
        }
    } else {
        ProcessError::Spawn {
            command: spec.to_display_string(),
            source,
        }
    }
}

/// Errors produced while building or executing a child process.
#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    /// The program is not installed or not on `PATH`.
    #[error("`{program}` was not found - is it installed and on your PATH?")]
    NotFound {
        /// The program that could not be located.
        program: String,
    },
    /// The process could not be started.
    #[error("failed to execute `{command}`")]
    Spawn {
        /// The rendered invocation.
        command: String,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// The process ran but exited unsuccessfully.
    #[error("`{command}` failed ({status})\n{stderr}")]
    Failed {
        /// The rendered invocation.
        command: String,
        /// How the process terminated.
        status: String,
        /// Captured standard error.
        stderr: String,
    },
    /// A command needed the project's directory name but the path has none.
    #[error("project path has no directory name: {path}")]
    NoDirectoryName {
        /// The offending path.
        path: PathBuf,
    },
}

#[cfg(test)]
pub(crate) mod fake {
    use super::{CommandOutput, CommandRunner, CommandSpec, IoPolicy, ProcessError};
    use std::cell::RefCell;
    use std::collections::VecDeque;

    #[derive(Debug, Default)]
    pub(crate) struct FakeRunner {
        queued: RefCell<VecDeque<CommandOutput>>,
        executed: RefCell<Vec<(CommandSpec, IoPolicy)>>,
        spawned: RefCell<Vec<CommandSpec>>,
    }

    impl FakeRunner {
        pub(crate) fn new() -> Self {
            Self::default()
        }

        pub(crate) fn push_stdout(&self, stdout: &str) -> &Self {
            self.queued.borrow_mut().push_back(CommandOutput {
                code: Some(0),
                stdout: stdout.to_owned(),
                stderr: String::new(),
            });
            self
        }

        pub(crate) fn push_failure(&self, code: i32, stderr: &str) -> &Self {
            self.queued.borrow_mut().push_back(CommandOutput {
                code: Some(code),
                stdout: String::new(),
                stderr: stderr.to_owned(),
            });
            self
        }

        pub(crate) fn executed(&self) -> Vec<CommandSpec> {
            self.executed
                .borrow()
                .iter()
                .map(|(spec, _)| spec.clone())
                .collect()
        }

        pub(crate) fn policies(&self) -> Vec<IoPolicy> {
            self.executed.borrow().iter().map(|(_, io)| *io).collect()
        }

        pub(crate) fn spawned(&self) -> Vec<CommandSpec> {
            self.spawned.borrow().clone()
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, spec: &CommandSpec, io: IoPolicy) -> Result<CommandOutput, ProcessError> {
            self.executed.borrow_mut().push((spec.clone(), io));
            Ok(self
                .queued
                .borrow_mut()
                .pop_front()
                .unwrap_or(CommandOutput {
                    code: Some(0),
                    stdout: String::new(),
                    stderr: String::new(),
                }))
        }

        fn spawn_detached(&self, spec: &CommandSpec) -> Result<(), ProcessError> {
            self.spawned.borrow_mut().push(spec.clone());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_invocation_for_diagnostics() {
        let spec = CommandSpec::new("cargo")
            .args(["run", "--release"])
            .arg("/tmp/my project");

        assert_eq!(
            spec.to_display_string(),
            "cargo run --release \"/tmp/my project\""
        );
    }

    #[test]
    fn records_program_arguments_and_directory() {
        let spec = CommandSpec::new("javac")
            .arg("Main.java")
            .current_dir("/tmp/day01");

        assert_eq!(spec.program(), "javac");
        assert_eq!(spec.arguments(), ["Main.java"]);
        assert_eq!(spec.working_dir(), Some(Path::new("/tmp/day01")));
    }

    #[test]
    fn reports_missing_programs_distinctly() {
        let spec = CommandSpec::new("definitely-not-a-real-program-9271");

        let error = SystemRunner
            .run(&spec, IoPolicy::SILENT)
            .expect_err("program should not exist");

        assert!(
            matches!(error, ProcessError::NotFound { .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn captures_output_of_real_processes() {
        let spec = CommandSpec::new("echo").arg("hello");

        let output = SystemRunner
            .run_checked(&spec, IoPolicy::SILENT)
            .expect("echo should succeed");

        assert_eq!(output.stdout.trim_end(), "hello");
        assert!(output.success());
    }

    #[test]
    fn run_checked_surfaces_stderr_of_failing_commands() {
        let runner = fake::FakeRunner::new();
        runner.push_failure(101, "error: could not compile\n");

        let error = runner
            .run_checked(&CommandSpec::new("cargo").arg("build"), IoPolicy::SILENT)
            .expect_err("command should fail");

        let message = error.to_string();
        assert!(message.contains("cargo build"), "{message}");
        assert!(message.contains("exit code 101"), "{message}");
        assert!(message.contains("could not compile"), "{message}");
    }

    #[test]
    fn run_checked_returns_output_on_success() {
        let runner = fake::FakeRunner::new();
        runner.push_stdout("42\n");

        let output = runner
            .run_checked(
                &CommandSpec::new("python3").arg("main.py"),
                IoPolicy::ANSWER,
            )
            .expect("command should succeed");

        assert_eq!(output.stdout, "42\n");
        assert_eq!(runner.executed().len(), 1);
        assert_eq!(runner.policies(), [IoPolicy::ANSWER]);
    }
}
