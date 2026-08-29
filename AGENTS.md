# AGENTS.md

This file provides guidance to coding agents working with code in this repository.

## What this is

`aoc-runtime` is a Rust CLI (`aoc`) that automates the Advent of Code loop: it works out which
puzzle you mean from the current directory, scaffolds the project, downloads the input, runs the
solution and submits the answers it prints. The crate is a library (`aoc_runtime`, `src/lib.rs`)
plus a thin binary (`src/main.rs`).

## Commands

```console
$ cargo test                      # unit + doc + end-to-end tests; no network, no toolchains needed
$ cargo test --lib resolve        # one module's unit tests
$ cargo test --lib the_default_day_is_clamped_after_the_event_ends   # one test by name
$ cargo test --test cli           # only the end-to-end tests in tests/cli.rs
$ cargo clippy --all-targets
$ cargo fmt --all --check
$ cargo doc --no-deps             # CI runs this with RUSTDOCFLAGS=-D warnings
$ cargo deny check                # licence/advisory audit, see deny.toml
```

CI sets `RUSTFLAGS: -D warnings`, so any warning is a build failure there. It also builds against
the MSRV declared as `rust-version` in `Cargo.toml` (1.88) — avoid APIs newer than that.

## Architecture

One pass, no hidden state:

```
Cli (clap)  ──►  Env::capture  ──►  Config::load  ──►  resolve::plan  ──►  Vec<Plan>  ──►  App::execute_all
src/cli.rs      src/env.rs          src/config.rs      src/resolve.rs            src/app{,/init,/run}.rs
```

* **`cli.rs`** only describes and parses arguments. Nothing there touches the clock, the filesystem
  or config.
* **`env.rs`** is the *only* place the library reads process environment variables or the current
  directory. `Clock` is the only source of today's date.
* **`resolve.rs`** is pure: `(Cli, Config, cwd, Clock) -> Vec<Plan>`, one plan per mode on the
  command line, in the order given. Precedence for year/day/language is explicit argument →
  recovered from the cwd via the template → date-based default, and the puzzle and project path are
  shared by every mode that names one. Each `Plan` variant carries exactly what its handler needs,
  so a mode that requires a language cannot be constructed without one and no downstream code
  re-checks; resolution is all-or-nothing, so a mode that cannot be planned fails before any mode
  executes. What a mode does not need it is never asked for: `clean` is planned without a puzzle,
  a language or a project, so an out-of-range year/day pairing is only an error for the modes that
  name a puzzle, and `--all`/`--yes` are refused unless `clean` is among the modes rather than
  being ignored.
* **`app.rs`** holds every injected dependency (`CommandRunner`, `AocClient`, `AnswerCache`,
  `InputStore`, `Reporter`, `Confirm`). `App::execute_all` walks the plans in order and stops at
  the first failure; `execute` dispatches one of them on `Plan`. `app/run.rs`, `app/init.rs` and
  `app/clean.rs` are the three real handlers.

### Everything external sits behind a trait

This is the load-bearing design decision: the whole tool is testable without a network, a language
toolchain or a real calendar.

| Trait | Real impl | Test impl |
| --- | --- | --- |
| `env::Clock` | `SystemClock` | `FixedClock` (public, not test-only) |
| `process::CommandRunner` | `SystemRunner` | `process::fake::FakeRunner` |
| `aoc::AocClient` | `aoc::live::LiveClient` | `aoc::fake::FakeClient` |
| `aoc::cache::AnswerCache` | `FileCache` | `cache::memory::MemoryCache` |
| `report::Reporter` | `TermReporter` | `report::recording::RecordingReporter` |
| `report::Confirm` | `TermConfirm` | `report::scripted::ScriptedConfirm` |
| `aoc::throttle::Timer` | `SystemTimer` | fake in that module's tests |

Handlers emit semantic `report::Event`s rather than printing, so tests assert on what happened, not
on ANSI escapes. Keep `Event::Data` (paths, URLs) on stdout and everything else on stderr —
`cd $(aoc path)` depends on it.

### Templates work in both directions

`template.rs` parses `~/projects/aoc/{{year}}/day{{pad day}}/{{language}}` once into `Segment`s.
`config.rs` refuses a `template_path` that is not absolute once `~` is expanded: a solution runs
with its project as the working directory, so a relative template would be read against the very
path it just rendered, and its matcher would never match an absolute cwd either.
The same segments are **rendered** into a project path and **compiled** (`template/matcher.rs`) into
a regex that recovers year/day/language *from* the cwd. Everything after each placeholder is wrapped
in an optional group, so standing partway along the path still recovers what precedes it. Detection
is best-effort — only explicit CLI arguments produce hard errors; a detected value that is out of
range is silently dropped in favour of the default.

### Validated coordinates

`puzzle.rs` uses newtypes with private fields and fallible constructors, so an out-of-range value
cannot reach path rendering or the API. `Puzzle::new` additionally validates the *pairing*: events
before `Year::FIRST_SHORT` (2025) run 25 puzzles, and from 2025 on only 12 (`Day::LAST_SHORT`).

### The answer protocol

`answer.rs` classifies a solution's stdout: exactly one non-blank line → part one; exactly two →
parts one and two; anything else (empty, blank line, three or more lines) → `Outcome::Raw`, printed
verbatim with nothing submitted. Solutions read their input from `../input.txt`, one per day shared
across languages — a symlink into the state directory, so it reads like a plain file and costs
nothing to recreate.

### Adding a language

`language.rs` is deliberately arranged so a new language is one `Language` variant plus one arm in
each of `name`, `entry_file`, `scaffold`, `commands` and `build_directory`. `scaffold` is the one-off project-creation
command, `None` when an empty entry file is enough; `commands` takes a `Layout { project, artifacts }`
and returns `LanguageCommands { build, run, run_fallback }`, `build` being `None` for an interpreted
language. Working directory is applied to all of them centrally by `with_working_dir`, and it is
always the *project* — a solution reads `../input.txt` no matter where its binary ended up.

A compiled language builds optimized once and is then invoked directly, with no build tool left in
the loop; only a tool that cannot hand over a runnable artifact (`javac`) keeps driving the run.
Where the artifact's name has to be guessed — cargo and dotnet both name it after the project
directory — `run_fallback` hands the run back to the tool rather than guessing harder; a fallback
that rebuilds must stay quiet, because its stdout is read as the solution's answers.
`build::executable` is the one place the platform's executable suffix is written down, so the
compiler and the run cannot disagree about the name of what was built.

### Build output

`build.rs` is the other half of `env.state_dir`'s job: `BuildStore::dir(puzzle, language)` names
`$XDG_STATE_HOME/aoc/builds/<year>-<day>/<language>`, keyed by `Puzzle::slug` exactly as
`InputStore` and `FileCache` key inputs and answers. It holds output for the languages compiled *by hand* (`java`, `go`, `c`,
`cpp`), which have no output directory of their own — their `commands` arm must aim every output
path at `layout.artifacts`.

`BuildStore::clear` empties the whole tree; it is what a bare `aoc clean` does, and it is safe
precisely because everything under it is regenerable. `InputStore` and `AnswerCache` grew the same
`clear` for `clean --all`, and each store owns its own root so nothing else repeats the
`join("builds")`/`join("inputs")`/`join("answers")` literals; the removal itself is
`store::remove_tree`, the one place where "a tree that was never created is already empty" is
written down. `clean --all` asks only
when an input or an answer is actually stored, so an unattended run with nothing to remove is not
refused.

`rust` and `csharp` deliberately build inside the project, into the `target/` and `bin/` their own
tooling owns. Redirecting them buys nothing: rust-analyzer and the C# language server build into
those directories regardless, so the project would carry one anyway and the state directory a
duplicate. `Language::build_directory` is the single place that split is written down; `dir` does no
I/O and `app/run.rs` creates a directory only when that method names one, so neither a managed nor
an interpreted language leaves an empty one behind.

### Errors

Each module owns a typed error (`ConfigError`, `ResolveError`, `ProcessError`, `ApiError`,
`TemplateError`, `EnvError`); `error::Error` is the union handlers return, mostly via
`#[error(transparent)]`. Use `IoResultExt::io_context("verb phrase", path)` for filesystem failures
so the message names both the action and the path. `error::report` walks the `source` chain.

## Automation etiquette — do not weaken these

The tool follows the Advent of Code [automation guidelines](https://www.reddit.com/r/adventofcode/wiki/faqs/automation),
and the README documents these guarantees to users. Treat them as invariants:

* `aoc::live::LiveClient` is the **only** code that contacts adventofcode.com, and every request
  goes through `Throttle::acquire` first (`aoc/throttle.rs`, 5 s minimum gap, persisted to the state
  directory so it holds *across* invocations). The throttle sits in `live.rs`'s own
  `impl Transport for ThrottledTransport`, not at the call sites, so it also covers requests
  `aoc_api` makes on its own — `submit` reads the puzzle page when a part is already solved.
* Every request carries `live::IDENTIFICATION` as its `User-Agent`, installed once when the HTTP
  client is built. Keep it a valid header value: ASCII, no control characters.
* An input is downloaded at most once, ever. `App::ensure_input` asks the client only when
  `aoc::input::InputStore` does not already hold the day; the stored copy lives in the state
  directory and each project's `input.txt` is a symlink into it, so deleting or re-scaffolding a
  project costs a link rather than a request. Do not "fix" a missing `input.txt` by refetching.
* Accepted answers are cached (`aoc/cache.rs`) so a solved part is verified locally instead of
  re-submitted.
* With no session cookie there is no client at all — a run provably cannot make a request.
* `aoc open` hands the puzzle URL to the OS link handler (`open::that`) and makes no request of
  its own, so it needs no throttle and does not widen the rule above. Keep it that way: it is the
  user's browser visiting the site, not the tool.
* `aoc clean` never removes `throttle::LAST_REQUEST_FILE`. The stamp sits at the root of the state
  directory rather than inside `builds/`, `inputs/` or `answers/`, so `app/clean.rs` preserves it
  by construction — do not "tidy" that into removing the state directory wholesale. `clean --all`
  does delete inputs, which is the one way the download-once rule is ever undone; that is why it
  is the only thing in the tool that asks the user first (`report::Confirm`), and why it refuses
  outright when there is no terminal to ask on and no `--yes`.
* The cookie is never printed. `Config` and `App` both have hand-written `Debug` impls that report
  `has_cookie`/`has_client` instead of the value; do not derive `Debug` on anything that can hold
  the cookie.

Also note: `LiveClient` is the only module allowed to mention `aoc_api` or `tokio`. It drives a
current-thread runtime to completion so the rest of the crate stays synchronous. `aoc.rs` owns this
crate's own `Verdict`, `Hint` and `ApiError`; `live.rs` translates the upstream types into them
(`verdict`, `recognised`, `translate`, `coordinates`), so nothing downstream depends on `aoc_api`.
All three upstream enums are `#[non_exhaustive]` — keep the catch-all arms honest rather than
guessing at a variant a newer client introduced.

## Conventions

* Lints in `Cargo.toml` are strict: `unsafe_code = "forbid"`, `missing_docs`/`unreachable_pub` warn,
  clippy `all` denied and `pedantic` warned, and **`unwrap`/`expect`/`panic` are denied**.
  `clippy.toml` re-allows them in tests only. In library code, model the failure instead.
* Every public item needs a doc comment; fallible public functions need an `# Errors` section
  (`cargo doc` is warning-free in CI).
* Test naming is behavioural and sentence-like — `a_cooldown_stops_before_the_second_part`,
  `unknown_keys_produce_a_warning_instead_of_silence`. Follow that style.
* Unit tests live beside the code in `#[cfg(test)] mod tests`, with shared fakes in sibling
  `#[cfg(test)] pub(crate) mod {fake,memory,recording}` modules and a `Harness` in `app::testing`.
  `tests/cli.rs` drives the real binary through `tests/support::Fixture`, which points
  `AOC_CONFIG_DIR`/`XDG_STATE_HOME` at a temp tree and clears `AOC_SESSION` — end-to-end tests must
  stay hermetic. Anything needing `cargo`/`dotnet`/`javac` belongs in a unit test with `FakeRunner`.
* Doc examples in `lib.rs`, `template.rs` and `answer.rs` are compiled and run by `cargo test`;
  update them when the API they show changes.
* Commit messages are Conventional Commits — release-plz parses them (`release-plz.toml`) to build
  the changelog and decide version bumps; `!`/`BREAKING CHANGE` is protected. Releases are automatic:
  merging the generated release PR publishes to crates.io, tags `vX.Y.Z` and attaches binaries.
