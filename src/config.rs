//! Loading and validating `config.yaml`.
//!
//! ```yaml
//! template_path: "~/projects/aoc/{{year}}/day{{pad day}}/{{language}}"
//! cookie: "<advent of code session cookie>"
//! editor: "code"
//! ```
//!
//! Only `template_path` is required. The template is parsed - and therefore
//! validated - at load time rather than on first use.

use crate::{env::Env, template::Template};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

/// The default editor launched by `aoc code`.
pub const DEFAULT_EDITOR: &str = "code";

/// Validated configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// The parsed project path template.
    pub template: Template,
    /// The Advent of Code session cookie, if one is available.
    pub cookie: Option<String>,
    /// The command launched by `aoc code`.
    pub editor: String,
    /// The directory the configuration was loaded from.
    pub config_dir: PathBuf,
}

/// A non-fatal problem noticed while loading configuration.
pub type Warning = String;

#[derive(Debug, Deserialize)]
struct RawConfig {
    template_path: String,
    #[serde(default)]
    cookie: Option<String>,
    #[serde(default)]
    editor: Option<String>,
    #[serde(flatten)]
    unknown: BTreeMap<String, serde_yaml_ng::Value>,
}

impl Config {
    /// Loads and validates the configuration described by `env`.
    ///
    /// Returns the configuration together with any non-fatal warnings, such as
    /// unrecognised keys - a misspelled `cookies:` would otherwise silently
    /// disable submission.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] if the file is missing, unreadable, not valid
    /// YAML, or contains an invalid `template_path`.
    pub fn load(env: &Env) -> Result<(Self, Vec<Warning>), ConfigError> {
        let contents = read_config(&env.config_file)?;
        Self::from_yaml(&contents, env)
    }

    /// Parses configuration from a YAML document, resolving paths and the
    /// session cookie against `env`.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] if the document is not valid YAML or the
    /// `template_path` is not a valid template.
    pub fn from_yaml(contents: &str, env: &Env) -> Result<(Self, Vec<Warning>), ConfigError> {
        let raw: RawConfig =
            serde_yaml_ng::from_str(contents).map_err(|source| ConfigError::Parse {
                path: env.config_file.clone(),
                source,
            })?;

        let mut warnings = Vec::new();
        for key in raw.unknown.keys() {
            warnings.push(format!(
                "ignoring unknown key `{key}` in {}",
                env.config_file.display()
            ));
        }

        let template_path = expand_home(&raw.template_path, &env.home);
        let template = Template::parse(&template_path).map_err(|source| ConfigError::Template {
            path: env.config_file.clone(),
            source,
        })?;

        let cookie = env
            .session_cookie
            .clone()
            .or(raw.cookie)
            .map(|cookie| cookie.trim().to_owned())
            .filter(|cookie| !cookie.is_empty());

        Ok((
            Self {
                template,
                cookie,
                editor: raw
                    .editor
                    .map(|editor| editor.trim().to_owned())
                    .filter(|editor| !editor.is_empty())
                    .unwrap_or_else(|| DEFAULT_EDITOR.to_owned()),
                config_dir: env.config_dir.clone(),
            },
            warnings,
        ))
    }
}

fn read_config(path: &Path) -> Result<String, ConfigError> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Err(ConfigError::NotFound {
            path: path.to_path_buf(),
        }),
        Err(source) => Err(ConfigError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn expand_home(path: &str, home: &Path) -> String {
    let expanded = match path {
        "~" => home.to_path_buf(),
        _ => match path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
            Some(rest) => home.join(rest),
            None => return path.to_owned(),
        },
    };

    expanded.to_string_lossy().into_owned()
}

/// Errors produced while loading configuration.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// No configuration file exists.
    #[error(
        "no config file at {path}\n\n\
         create it with at least a project path template, for example:\n  \
         template_path: \"~/projects/aoc/{{{{year}}}}/day{{{{pad day}}}}/{{{{language}}}}\""
    )]
    NotFound {
        /// Where the file was expected.
        path: PathBuf,
    },
    /// The configuration file could not be read.
    #[error("failed to read config file {path}")]
    Read {
        /// The file that could not be read.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// The configuration file is not valid YAML, or is missing a required key.
    #[error("failed to parse config file {path}")]
    Parse {
        /// The offending file.
        path: PathBuf,
        /// The underlying deserialisation error.
        #[source]
        source: serde_yaml_ng::Error,
    },
    /// The `template_path` is not a valid template.
    #[error("invalid `template_path` in {path}")]
    Template {
        /// The offending file.
        path: PathBuf,
        /// The underlying template error.
        #[source]
        source: crate::template::TemplateError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{language::Language, puzzle::Day, puzzle::Year, template::Params};

    fn env() -> Env {
        Env {
            home: PathBuf::from("/home/tester"),
            config_dir: PathBuf::from("/home/tester/.config/aoc"),
            config_file: PathBuf::from("/home/tester/.config/aoc/config.yaml"),
            state_dir: PathBuf::from("/home/tester/.local/state/aoc"),
            cwd: PathBuf::from("/home/tester"),
            session_cookie: None,
        }
    }

    fn load(yaml: &str) -> Result<(Config, Vec<Warning>), ConfigError> {
        Config::from_yaml(yaml, &env())
    }

    fn project_path(config: &Config) -> PathBuf {
        config.template.render(Params {
            year: Year::new(2024).expect("valid year"),
            day: Day::new(7).expect("valid day"),
            language: Language::Rust,
        })
    }

    #[test]
    fn loads_a_minimal_config() {
        let (config, warnings) =
            load("template_path: \"/aoc/{{year}}/day{{pad day}}/{{language}}\"")
                .expect("config should load");

        assert!(warnings.is_empty());
        assert_eq!(config.cookie, None);
        assert_eq!(config.editor, DEFAULT_EDITOR);
        assert_eq!(project_path(&config), Path::new("/aoc/2024/day07/rust"));
    }

    #[test]
    fn expands_a_leading_tilde() {
        let (config, _) = load("template_path: \"~/aoc/{{year}}/day{{pad day}}/{{language}}\"")
            .expect("config should load");

        assert_eq!(
            project_path(&config),
            Path::new("/home/tester/aoc/2024/day07/rust")
        );
    }

    #[test]
    fn leaves_a_tilde_elsewhere_alone() {
        let (config, _) = load("template_path: \"/aoc/~backup/{{year}}/day{{day}}\"")
            .expect("config should load");

        assert_eq!(project_path(&config), Path::new("/aoc/~backup/2024/day7"));
    }

    #[test]
    fn reads_the_cookie_and_editor() {
        let (config, _) = load(
            "template_path: \"/aoc/{{year}}/day{{day}}\"\ncookie: \"  abc123  \"\neditor: nvim\n",
        )
        .expect("config should load");

        assert_eq!(config.cookie.as_deref(), Some("abc123"));
        assert_eq!(config.editor, "nvim");
    }

    #[test]
    fn an_empty_cookie_is_no_cookie() {
        let (config, _) = load("template_path: \"/aoc/{{year}}/day{{day}}\"\ncookie: \"\"\n")
            .expect("config should load");

        assert_eq!(config.cookie, None);
    }

    #[test]
    fn the_environment_cookie_wins() {
        let mut env = env();
        env.session_cookie = Some("from-env".to_owned());

        let (config, _) = Config::from_yaml(
            "template_path: \"/aoc/{{year}}/day{{day}}\"\ncookie: from-file\n",
            &env,
        )
        .expect("config should load");

        assert_eq!(config.cookie.as_deref(), Some("from-env"));
    }

    #[test]
    fn unknown_keys_produce_a_warning_instead_of_silence() {
        let (config, warnings) =
            load("template_path: \"/aoc/{{year}}/day{{day}}\"\ncookies: oops\n")
                .expect("config should load");

        assert_eq!(config.cookie, None);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("cookies"), "{warnings:?}");
    }

    #[test]
    fn a_missing_template_path_is_an_error() {
        let error = load("cookie: abc123").expect_err("template_path is required");

        assert!(matches!(error, ConfigError::Parse { .. }), "got {error:?}");
    }

    #[test]
    fn an_invalid_template_names_the_config_file() {
        let error = load("template_path: \"/aoc/{{year}}\"").expect_err("day is missing");

        assert!(
            matches!(error, ConfigError::Template { .. }),
            "got {error:?}"
        );
        assert!(error.to_string().contains("config.yaml"), "{error}");
    }

    #[test]
    fn malformed_yaml_is_an_error() {
        let error = load("template_path: [unclosed").expect_err("yaml is malformed");

        assert!(matches!(error, ConfigError::Parse { .. }), "got {error:?}");
    }

    #[test]
    fn a_missing_file_explains_how_to_create_one() {
        let error = read_config(Path::new("/nonexistent/aoc/config.yaml"))
            .expect_err("file should not exist");

        assert!(
            matches!(error, ConfigError::NotFound { .. }),
            "got {error:?}"
        );
        assert!(error.to_string().contains("template_path"), "{error}");
    }
}
