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
//!
//! The cookie may instead live in a [`COOKIE_FILE_NAME`] file in the
//! configuration directory, holding nothing but the cookie, so the
//! configuration itself carries no secret and can be committed alongside other
//! dotfiles.

use crate::{env::Env, template::Template};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

/// The default editor launched by `aoc code`.
pub const DEFAULT_EDITOR: &str = "code";

/// The file read from [`Env::config_dir`] as a raw session cookie when the
/// configuration itself does not carry one.
pub const COOKIE_FILE_NAME: &str = "COOKIE";

/// Validated configuration.
#[derive(Clone)]
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

/// Reports only whether a cookie is present, so no `{:?}` of a [`Config`] -
/// or of anything holding one - can leak the session cookie into a log or a
/// panic message.
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("template", &self.template)
            .field("has_cookie", &self.cookie.is_some())
            .field("editor", &self.editor)
            .field("config_dir", &self.config_dir)
            .finish()
    }
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
    /// With no cookie in the environment or the file itself, the
    /// [`COOKIE_FILE_NAME`] file in [`Env::config_dir`] is read as one. A cookie
    /// file that exists but cannot be read is a warning rather than an error,
    /// so it cannot stop `aoc path`, `aoc code` or any other command that
    /// needs no cookie at all.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] if the file is missing, unreadable, not valid
    /// YAML, or contains a relative or otherwise invalid `template_path`.
    pub fn load(env: &Env) -> Result<(Self, Vec<Warning>), ConfigError> {
        let contents = read_config(&env.config_file)?;
        let (mut config, mut warnings) = Self::from_yaml(&contents, env)?;

        if config.cookie.is_none() {
            config.cookie = read_cookie(&env.config_dir.join(COOKIE_FILE_NAME), &mut warnings);
        }

        Ok((config, warnings))
    }

    /// Parses configuration from a YAML document, resolving paths and the
    /// session cookie against `env`. Unlike [`Config::load`], this does not
    /// fall back to the cookie file.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] if the document is not valid YAML, or the
    /// `template_path` is relative or not a valid template.
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
        // Everything a solution does happens with the project as the working
        // directory, so a relative template would be resolved against the
        // project it just named - and would never match the working directory
        // it is supposed to recover a puzzle from either.
        if !Path::new(&template_path).is_absolute() {
            return Err(ConfigError::RelativeTemplate {
                path: env.config_file.clone(),
                template: raw.template_path,
            });
        }

        let template = Template::parse(&template_path).map_err(|source| ConfigError::Template {
            path: env.config_file.clone(),
            source,
        })?;

        let cookie = trimmed(env.session_cookie.clone().or(raw.cookie));

        Ok((
            Self {
                template,
                cookie,
                editor: trimmed(raw.editor).unwrap_or_else(|| DEFAULT_EDITOR.to_owned()),
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

fn read_cookie(path: &Path, warnings: &mut Vec<Warning>) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(contents) => trimmed(Some(contents)),
        Err(source) if source.kind() == io::ErrorKind::NotFound => None,
        Err(source) => {
            warnings.push(format!(
                "ignoring unreadable cookie file {}: {source}",
                path.display()
            ));
            None
        }
    }
}

/// Trims a configured value, treating a blank one as absent. Applied to every
/// source a cookie can come from, so they cannot drift apart.
fn trimmed(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
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
///
/// Non-exhaustive: a future release may add a variant without that being a
/// breaking change, so match with a catch-all arm.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
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
    /// The `template_path` does not start at the root of the filesystem.
    #[error(
        "`template_path` in {path} is relative: `{template}`\n\n\
         it must be absolute - start it with `~/` or `/` - because a solution \
         runs with its project as the working directory, and a relative \
         template would be read against whichever directory that happens to be"
    )]
    RelativeTemplate {
        /// The offending file.
        path: PathBuf,
        /// The template as it was written, before `~` was expanded.
        template: String,
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
    use crate::{
        env::CONFIG_FILE_NAME, language::Language, puzzle::Day, puzzle::Year, template::Params,
    };

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

    /// Writes `config.yaml`, and a cookie file when one is given, into a
    /// throwaway configuration directory, and points an [`Env`] at it. The
    /// directory is handed back so it outlives the load.
    fn fixture(yaml: &str, cookie_file: Option<&str>) -> (tempfile::TempDir, Env) {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(dir.path().join(CONFIG_FILE_NAME), yaml).expect("write config");
        if let Some(cookie) = cookie_file {
            fs::write(dir.path().join(COOKIE_FILE_NAME), cookie).expect("write cookie file");
        }

        let mut env = env();
        env.config_dir = dir.path().to_path_buf();
        env.config_file = dir.path().join(CONFIG_FILE_NAME);

        (dir, env)
    }

    /// Loads a configuration written to a throwaway directory by [`fixture`].
    fn load_from_disk(
        yaml: &str,
        cookie_file: Option<&str>,
    ) -> Result<(Config, Vec<Warning>), ConfigError> {
        let (_dir, env) = fixture(yaml, cookie_file);

        Config::load(&env)
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
    fn refuses_a_relative_template() {
        // A solution runs with its project as the working directory, so a
        // relative template is read against the very path it just named.
        for source in [
            "aoc/{{year}}/day{{pad day}}/{{language}}",
            "./aoc/{{year}}/day{{pad day}}",
            "../aoc/{{year}}/day{{pad day}}",
        ] {
            let error = load(&format!("template_path: \"{source}\""))
                .expect_err("a relative template cannot be resolved");

            assert!(
                matches!(&error, ConfigError::RelativeTemplate { template, .. } if template == source),
                "{source}: {error:?}"
            );
            assert!(error.to_string().contains("absolute"), "{error}");
        }
    }

    #[test]
    fn a_relative_template_is_refused_before_its_placeholders_are_read() {
        // The path being unusable is the more actionable of the two problems.
        let error = load("template_path: \"aoc/{{year}}\"").expect_err("relative, and dayless");

        assert!(
            matches!(error, ConfigError::RelativeTemplate { .. }),
            "{error:?}"
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
    fn debug_output_redacts_the_cookie() {
        let (config, _) =
            load("template_path: \"/aoc/{{year}}/day{{day}}\"\ncookie: super-secret\n")
                .expect("config should load");

        let debug = format!("{config:?}");

        assert!(!debug.contains("super-secret"), "{debug}");
        assert!(debug.contains("has_cookie: true"), "{debug}");
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
    fn the_cookie_file_stands_in_for_a_missing_cookie_key() {
        let (config, _) = load_from_disk(
            "template_path: \"/aoc/{{year}}/day{{day}}\"\n",
            Some("from-file\n"),
        )
        .expect("config should load");

        assert_eq!(config.cookie.as_deref(), Some("from-file"));
    }

    #[test]
    fn the_configured_cookie_wins_over_the_cookie_file() {
        let (config, _) = load_from_disk(
            "template_path: \"/aoc/{{year}}/day{{day}}\"\ncookie: from-config\n",
            Some("from-file"),
        )
        .expect("config should load");

        assert_eq!(config.cookie.as_deref(), Some("from-config"));
    }

    #[test]
    fn the_environment_cookie_wins_over_the_cookie_file() {
        let (_dir, mut env) = fixture(
            "template_path: \"/aoc/{{year}}/day{{day}}\"\n",
            Some("from-file"),
        );
        env.session_cookie = Some("from-env".to_owned());

        let (config, _) = Config::load(&env).expect("config should load");

        assert_eq!(config.cookie.as_deref(), Some("from-env"));
    }

    #[test]
    fn a_blank_cookie_file_is_no_cookie() {
        let (config, _) = load_from_disk(
            "template_path: \"/aoc/{{year}}/day{{day}}\"\n",
            Some("  \n"),
        )
        .expect("config should load");

        assert_eq!(config.cookie, None);
    }

    #[test]
    fn no_cookie_file_is_no_cookie() {
        let (config, _) = load_from_disk("template_path: \"/aoc/{{year}}/day{{day}}\"\n", None)
            .expect("config should load");

        assert_eq!(config.cookie, None);
    }

    #[test]
    fn an_unreadable_cookie_file_warns_instead_of_stopping_every_command() {
        let (dir, env) = fixture("template_path: \"/aoc/{{year}}/day{{day}}\"\n", None);
        fs::create_dir(dir.path().join(COOKIE_FILE_NAME)).expect("create cookie directory");

        let (config, warnings) = Config::load(&env).expect("an unreadable cookie is not fatal");

        assert_eq!(config.cookie, None);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains(COOKIE_FILE_NAME), "{warnings:?}");
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
