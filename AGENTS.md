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
Cli (clap)  ──►  Env::capture  ──►  Config::load  ──►  resolve::plan  ──►  Plan  ──►  App::execute
src/cli.rs      src/env.rs          src/config.rs      src/resolve.rs            src/app{,/init,/run}.rs
```

* **`cli.rs`** only describes and parses arguments. Nothing there touches the clock, the filesystem
  or config.
* **`env.rs`** is the *only* place the library reads process environment variables or the current
  directory. `Clock` is the only source of today's date.
* **`resolve.rs`** is pure: `(Cli, Config, cwd, Clock) -> Plan`. Precedence for year/day/language is
  explicit argument → recovered from the cwd via the template → date-based default. Each `Plan`
  variant carries exactly what its handler needs, so a mode that requires a language cannot be
  constructed without one and no downstream code re-checks.
* **`app.rs`** holds every injected dependency (`CommandRunner`, `AocClient`, `AnswerCache`,
  `InputStore`, `Reporter`) and dispatches on `Plan`. `app/run.rs` and `app/init.rs` are the two
  real handlers.

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
| `aoc::throttle::Timer` | `SystemTimer` | fake in that module's tests |

Handlers emit semantic `report::Event`s rather than printing, so tests assert on what happened, not
on ANSI escapes. Keep `Event::Data` (paths, URLs) on stdout and everything else on stderr —
`cd $(aoc path)` depends on it.

### Templates work in both directions

`template.rs` parses `~/projects/aoc/{{year}}/day{{pad day}}/{{language}}` once into `Segment`s.
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
each of `name`, `base_extension`, `entry_file` and `commands`. `commands` returns
`LanguageCommands { init, build, run, run_fallback }`; `init`/`build` are `None` when the language
needs neither. Working directory is applied to all of them centrally by `with_working_dir`.

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
