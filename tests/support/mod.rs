//! A hermetic fixture for driving the real `aoc` binary.
//!
//! Every command runs against a throwaway configuration directory inside a
//! temporary tree, with no session cookie, so the tests cannot reach the
//! network and cannot see the developer's own `~/.config/aoc`.

use aoc_runtime::language::BASE_DIR_NAME;
use assert_cmd::Command;
use std::{
    fs,
    path::{MAIN_SEPARATOR_STR, Path, PathBuf},
};
use tempfile::TempDir;

pub(crate) struct Fixture {
    // Held for its `Drop`: the temporary tree is removed with it. The paths the
    // tests use come from `path`, which is the resolved form of the same
    // directory.
    _root: TempDir,
    path: PathBuf,
    template: String,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        Self::with_template("{root}/aoc/{{year}}/day{{pad day}}/{{language}}")
    }

    pub(crate) fn with_template(template: &str) -> Self {
        let root = tempfile::tempdir().expect("temp dir");
        let path = resolved(root.path());
        let config_dir = path.join("config");
        fs::create_dir_all(&config_dir).expect("create config dir");

        let template = template
            .replace("{root}", &path.to_string_lossy())
            .replace('/', MAIN_SEPARATOR_STR);

        fs::write(
            config_dir.join("config.yaml"),
            format!("template_path: {}\n", yaml_string(&template)),
        )
        .expect("write config");

        Self {
            _root: root,
            path,
            template,
        }
    }

    pub(crate) fn root(&self) -> PathBuf {
        self.path.clone()
    }

    pub(crate) fn config_dir(&self) -> PathBuf {
        self.path.join("config")
    }

    pub(crate) fn solutions(&self) -> PathBuf {
        self.path.join("aoc")
    }

    /// The project directory below the solutions tree, assembled one component
    /// at a time so the expectation carries the platform's own separator, just
    /// as the rendered template does.
    pub(crate) fn project(&self, relative: &str) -> PathBuf {
        relative
            .split('/')
            .fold(self.solutions(), |path, component| path.join(component))
    }

    /// The template this fixture configured, for tests that rewrite the config
    /// file and want to keep it.
    pub(crate) fn template(&self) -> &str {
        &self.template
    }

    pub(crate) fn write_config(&self, yaml: &str) {
        fs::write(self.config_dir().join("config.yaml"), yaml).expect("write config");
    }

    pub(crate) fn write_base_file(&self, language: &str, contents: &str) {
        let base = self.config_dir().join(BASE_DIR_NAME);
        fs::create_dir_all(&base).expect("create base directory");
        fs::write(base.join(language), contents).expect("write base file");
    }

    pub(crate) fn command(&self) -> Command {
        self.command_in(&self.root())
    }

    pub(crate) fn command_in(&self, cwd: &Path) -> Command {
        fs::create_dir_all(cwd).expect("create working directory");

        let mut command = Command::cargo_bin("aoc").expect("the aoc binary should be built");
        command
            .current_dir(cwd)
            .env("AOC_CONFIG_DIR", self.config_dir())
            .env("XDG_STATE_HOME", self.path.join("state"))
            .env_remove("AOC_SESSION")
            .env("NO_COLOR", "1");

        command
    }

    pub(crate) fn command_without_config(&self) -> Command {
        let mut command = Command::cargo_bin("aoc").expect("the aoc binary should be built");
        command
            .current_dir(&self.path)
            .env("AOC_CONFIG_DIR", self.path.join("nowhere"))
            .env_remove("AOC_SESSION")
            .env("NO_COLOR", "1");

        command
    }
}

/// Quotes a value as a single-quoted YAML scalar. A Windows path is full of
/// backslashes, which a double-quoted scalar would read as escape sequences.
pub(crate) fn yaml_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// The path the operating system itself will report for a directory: on macOS
/// the temporary tree is reached through a symlink, and the working directory a
/// child process reports is always the resolved one, so a template built from
/// the unresolved path would never match it.
fn resolved(path: &Path) -> PathBuf {
    let canonical = path.canonicalize().expect("resolve temp dir");
    let text = canonical.to_string_lossy();

    // Windows canonicalisation hands back a verbatim `\\?\C:\...` path, which
    // cannot serve as a process working directory.
    PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text).to_owned())
}
