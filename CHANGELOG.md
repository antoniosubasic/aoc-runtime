# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

From the next release onwards this file is maintained automatically by
[release-plz](https://release-plz.dev) from the commit history.

## [0.9.0](https://github.com/antoniosubasic/aoc-runtime/compare/v0.8.0...v0.9.0) - 2026-08-29

### Added

- *(language)* [**breaking**] run the built binary and add six languages
- *(cli)* [**breaking**] empty the state directory with aoc clean

### Changed

- write the emptying rule down once

### Documentation

- describe what clean needs and what a fallback may print

### Fixed

- *(language)* keep build chatter out of the C# fallback's answers
- *(language)* spell a built binary the way the platform does
- *(report)* only ask when the question can be seen
- *(cli)* do not ask about removing nothing
- *(cli)* [**breaking**] plan clean without a puzzle and scope its flags to it
- *(config)* refuse a relative template_path
- *(config)* give advice a windows path can follow

## [0.8.0](https://github.com/antoniosubasic/aoc-runtime/compare/v0.7.0...v0.8.0) - 2026-08-29

### Added

- *(cli)* [**breaking**] run several modes in the order given
- *(cli)* open the puzzle in the default browser

### Dependencies

- *(deps)* bump aoc_api from 4.1.0 to 5.0.0

## [0.7.0](https://github.com/antoniosubasic/aoc-runtime/compare/v0.6.0...v0.7.0) - 2026-08-19

### Changed

- *(config)* [**breaking**] mark ConfigError non-exhaustive
- *(config)* trim configured values in one place
- *(language)* name the base directory with a constant

### Documentation

- *(config)* say where the cookie file is really looked for
- record the cookie guarantees the fixes introduced

### Fixed

- *(config)* keep the session cookie out of Debug output
- *(config)* [**breaking**] warn when the cookie file cannot be read, rather than stop
- *(env)* resolve a relative --config path against the current directory

## [0.6.0](https://github.com/antoniosubasic/aoc-runtime/compare/v0.5.1...v0.6.0) - 2026-08-19

### Added

- *(language)* [**breaking**] keep base files in base/ named after the language
- *(config)* fall back to a COOKIE file beside config.yaml

### Dependencies

- *(deps)* bump h2 from 0.4.15 to 0.4.16

## [0.5.0] - 2026-08-04

Rebuilt the tool from the ground up. The command line, the configuration file,
the template placeholders, the base files and the stdout answer protocol are
all unchanged, so existing setups keep working.

### Added

- Published to crates.io — `cargo install aoc-runtime` — with prebuilt binaries
  for Linux, macOS and Windows attached to every release.
- A library, `aoc_runtime`, alongside the `aoc` binary. Puzzle coordinates,
  templates, languages, the command runner and the Advent of Code client are a
  documented public API; the binary is reduced to argument parsing and wiring.
- A `LICENSE` — GPL-3.0-only — and the package metadata a published crate
  needs: description, repository, keywords and categories. The project
  previously stated no licence at all.
- `--version`, and help text for every flag and mode. Previously every entry in
  `--help` had an empty description.
- `--no-submit`, to run a solution without submitting its answers. There was
  previously no way to do this with a cookie configured.
- `--config <FILE>`, plus the `AOC_SESSION` and `AOC_CONFIG_DIR` environment
  variables. `AOC_SESSION` keeps the session cookie out of a plaintext file.
- `XDG_CONFIG_HOME` is honoured; `~/.config/aoc` remains the default so
  existing configurations keep working.
- An optional `editor` config key. `aoc code` no longer hard-codes `code`.
- Accepted answers are cached under `XDG_STATE_HOME`, so a solved puzzle is
  verified locally instead of being re-submitted on every run.
- Outbound requests are throttled to one every five seconds, as the Advent of
  Code automation guidelines ask. The last request is timestamped in the state
  directory, so the gap holds across invocations and a run in a shell loop
  cannot hammer the site.
- A README documenting the configuration schema, the template placeholders and
  the answer protocol, none of which were written down anywhere before.
- A test suite: unit tests plus end-to-end tests that need no network.

### Changed

- Solutions now run optimized. `cargo build --release` was followed by a plain
  `cargo run`, which discarded the release build and executed a debug binary.
  C# equivalently builds and runs in Release, and reuses the build output.
- Data goes to stdout and diagnostics to stderr, so `cd $(aoc path)` is safe.
- A solution's stderr streams through live instead of being swallowed.
- Failing to download the puzzle input warns instead of aborting the run.
- A cooldown from the site is reported rather than raised as an error, and no
  longer discards the reason it was rejected.
- Configuration is validated when it is loaded, so a bad `template_path` is
  reported with the offending placeholder instead of failing later with
  `failed to find 'year' in template path`.
- An unrecognised config key now warns. A misspelled `cookies:` used to be
  ignored silently, leaving submission mysteriously disabled.
- `aoc init` removes the directory it created if scaffolding fails, instead of
  leaving a half-built project that blocks the next attempt.
- The day is validated against its year. Advent of Code publishes 12 puzzles
  from 2025 on instead of 25, so `aoc -y 2025 -d 20` is now a usage error, a
  `day20` directory under a 2025 event is ignored, and the December default
  clamps to day 12.
- `{{language}}` is now optional in the template.
- Scaffolding Java and Python no longer shells out to `touch`.
- Python is run with `python3`, falling back to `python`.
- The minimum supported Rust version is now 1.88, declared in `Cargo.toml` and
  built against by CI. No MSRV was declared before.
- Replaced `serde_yml` with the maintained `serde_yaml_ng`; updated `aoc_api`
  to 3.0.4; dropped `strum`, `strum_macros`, `anyhow` and the unused parts of
  `tokio`.

### Fixed

- `--help` and `--version` work without a config file. Configuration used to be
  loaded before arguments were parsed, so a new user's first command failed.
- Solution output containing non-ASCII characters no longer mis-splits or
  panics. Newline offsets were counted in characters but used as byte indices.
- A `day10`–`day25` directory is recognised correctly. The pattern matched
  leftmost-first, so `day15` resolved to day 1 and pointed at the wrong project.
- The default day is clamped to the last day of the event. On 26–31 December
  `aoc url` produced `day/26`–`day/31`, bypassing the validation applied to
  explicit arguments.
- Working-directory detection works for any placeholder order, and is anchored
  so an unrelated four-digit directory elsewhere in the path cannot match.
- A template written as `{{ year }}` is detected as well as rendered. Rendering
  tolerated the spaces; detection did not.
- Detection accepts either path separator, so a template written with `/` still
  recovers the parameters from a Windows working directory, which the operating
  system always reports with backslashes.
- Output whose last line has no trailing newline is submitted correctly.
  `"42\n17"` used to be submitted whole as part one.
- A project path with no directory name is an error rather than a panic.

### Removed

- The Nix flake. `flake.nix` and `flake.lock` are gone, so `nix run` and
  `nix profile install` against this repository no longer work. Install with
  `cargo install aoc-runtime` or a prebuilt binary from a release instead.
- The tag-triggered release workflow, which merged into `main`, rewrote
  `Cargo.toml`, amended the merge commit and pushed. Releases now go through a
  release pull request; CI never writes to the repository.

<!-- previous releases were not tracked in a changelog; see the git history -->

[0.5.0]: https://github.com/antoniosubasic/aoc-runtime/compare/v0.4.0...v0.5.0
