//! A hermetic fixture for driving the real `aoc` binary.
//!
//! Every command runs against a throwaway configuration directory inside a
//! temporary tree, with no session cookie, so the tests cannot reach the
//! network and cannot see the developer's own `~/.config/aoc`.

use assert_cmd::Command;
use std::{fs, path::PathBuf};
use tempfile::TempDir;

pub(crate) struct Fixture {
    root: TempDir,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        Self::with_template("{root}/aoc/{{year}}/day{{pad day}}/{{language}}")
    }

    pub(crate) fn with_template(template: &str) -> Self {
        let root = tempfile::tempdir().expect("temp dir");
        let config_dir = root.path().join("config");
        fs::create_dir_all(&config_dir).expect("create config dir");

        let template = template.replace("{root}", &root.path().to_string_lossy());
        fs::write(
            config_dir.join("config.yaml"),
            format!("template_path: \"{template}\"\n"),
        )
        .expect("write config");

        Self { root }
    }

    pub(crate) fn root(&self) -> PathBuf {
        self.root.path().to_path_buf()
    }

    pub(crate) fn config_dir(&self) -> PathBuf {
        self.root.path().join("config")
    }

    pub(crate) fn solutions(&self) -> PathBuf {
        self.root.path().join("aoc")
    }

    pub(crate) fn write_config(&self, yaml: &str) {
        fs::write(self.config_dir().join("config.yaml"), yaml).expect("write config");
    }

    pub(crate) fn write_base_file(&self, name: &str, contents: &str) {
        fs::write(self.config_dir().join(name), contents).expect("write base file");
    }

    pub(crate) fn command(&self) -> Command {
        self.command_in(&self.root())
    }

    pub(crate) fn command_in(&self, cwd: &std::path::Path) -> Command {
        fs::create_dir_all(cwd).expect("create working directory");

        let mut command = Command::cargo_bin("aoc").expect("the aoc binary should be built");
        command
            .current_dir(cwd)
            .env("AOC_CONFIG_DIR", self.config_dir())
            .env("XDG_STATE_HOME", self.root.path().join("state"))
            .env_remove("AOC_SESSION")
            .env("NO_COLOR", "1");

        command
    }

    pub(crate) fn command_without_config(&self) -> Command {
        let mut command = Command::cargo_bin("aoc").expect("the aoc binary should be built");
        command
            .current_dir(self.root.path())
            .env("AOC_CONFIG_DIR", self.root.path().join("nowhere"))
            .env_remove("AOC_SESSION")
            .env("NO_COLOR", "1");

        command
    }
}
