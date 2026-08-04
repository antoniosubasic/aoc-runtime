# aoc-runtime

[![CI](https://github.com/antoniosubasic/aoc-runtime/actions/workflows/ci.yml/badge.svg)](https://github.com/antoniosubasic/aoc-runtime/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/aoc-runtime.svg)](https://crates.io/crates/aoc-runtime)
[![downloads](https://img.shields.io/crates/d/aoc-runtime.svg)](https://crates.io/crates/aoc-runtime)
[![license](https://img.shields.io/crates/l/aoc-runtime.svg)](LICENSE)

A runtime automation tool for [Advent of Code](https://adventofcode.com). It
works out which puzzle you mean from the directory you are standing in,
scaffolds the project, downloads the input, runs your solution and submits the
answers it prints.

```console
$ cd ~/projects/aoc/2024/day07/rust
$ aoc
123456789
1234567891011
```

Both answers green: both accepted.

## Install

```console
$ cargo install aoc-runtime
```

The binary is called `aoc`. Prebuilt archives for Linux, macOS and Windows are
attached to every [release](https://github.com/antoniosubasic/aoc-runtime/releases).

## Configure

Create `~/.config/aoc/config.yaml`:

```yaml
# required: where your solutions live
template_path: "~/projects/aoc/{{year}}/day{{pad day}}/{{language}}"

# optional: your adventofcode.com session cookie, needed to download inputs
# and submit answers
cookie: "53616c7465645f5f..."

# optional: what `aoc code` launches (default: code)
editor: "code"
```

| Key | Required | Default | Meaning |
| --- | --- | --- | --- |
| `template_path` | yes | | Path template for a solution directory. A leading `~` is expanded. |
| `cookie` | no | none | Session cookie. Without it, `aoc` never contacts adventofcode.com. |
| `editor` | no | `code` | Command run by `aoc code`. |

To find your session cookie, open adventofcode.com while logged in, and copy
the value of the `session` cookie from your browser's developer tools. It is a
credential: treat it like a password. If you would rather not keep it in a
file, set `AOC_SESSION` instead — it takes precedence over the config file.

### Template placeholders

| Placeholder | Expands to |
| --- | --- |
| `{{year}}` | The four-digit year, e.g. `2024` |
| `{{day}}` | The day, unpadded, e.g. `7` |
| `{{pad day}}` | The day, zero-padded to two digits, e.g. `07` |
| `{{language}}` | `rust`, `csharp`, `java` or `python` |

Whitespace inside the braces is insignificant, so `{{pad day}}` and
`{{ pad day }}` are the same placeholder. `{{year}}` and a day placeholder are
required; `{{language}}` is optional, though without it `aoc` cannot tell your
languages apart when reading the current directory.

The template is used in both directions. It builds the project path, and it is
also compiled into a pattern that recovers the year, day and language *from*
the directory you are in — which is why `aoc` on its own usually does the right
thing.

### Base files

If `~/.config/aoc/base.<ext>` exists, `aoc init` copies it over the generated
entry point, so every new project starts from your own boilerplate.

| Language | Base file | Copied to |
| --- | --- | --- |
| `rust` | `base.rs` | `src/main.rs` |
| `csharp` | `base.cs` | `Program.cs` |
| `java` | `base.java` | `Main.java` |
| `python` | `base.py` | `main.py` |

## Usage

```
aoc [OPTIONS] [MODE]
```

| Mode | What it does |
| --- | --- |
| `run` *(default)* | Build the solution, run it, submit the answers it prints |
| `init` | Create the project directory and scaffold a solution |
| `path` | Print the project directory |
| `code` | Open the project directory in your editor |
| `url` | Print the puzzle URL |

| Option | Description |
| --- | --- |
| `-y, --year <YEAR>` | Puzzle year. Defaults to the most recent event. |
| `-d, --day <DAY>` | Puzzle day, `1`–`25` (`1`–`12` from 2025 on). Defaults to today during December, otherwise `1`. |
| `-l, --language <LANGUAGE>` | `rust`, `csharp`, `java` or `python`. |
| `--no-submit` | Run the solution but do not submit its answers. |
| `--config <FILE>` | Use a specific config file. |
| `-h, --help` / `-V, --version` | |

Every mode except `url` needs a language.

### How values are resolved

For each of year, day and language, in order:

1. the command line argument, if given;
2. the value recovered from the current directory via the template;
3. a date-based default (the most recent event; today's day during December,
   otherwise day 1). Language has no default.

The day is checked against the year it belongs to: events up to 2024 run 25
puzzles, and from 2025 on they run 12. A day past the end of the event is a
usage error when you ask for it explicitly, and simply ignored when it came
from the working directory.

So from `~/projects/aoc/2024/day07/rust`, a bare `aoc` runs 2024 day 7 in Rust.
From `~/projects/aoc/2024`, `aoc -d 7 -l rust` fills in the rest.

### The answer protocol

Print your answers to stdout, one per line — part one first:

```
123456789
1234567891011
```

* **One line** → submitted as part one.
* **Two lines** → submitted as parts one and two.
* **Anything else** (no output, three or more lines, a blank line) → printed
  verbatim and nothing is submitted.

Correct answers print green and rejected ones red, with whatever else the site
said — that the answer is too high or too low, or how long to wait before trying
again — on stderr. An answer the site declined to judge at all prints yellow:
either something was submitted too recently, or it was not asking for an answer
to that part. A wait stops the run there, so the second part is not spent on a
submission that is certain to be refused.

Use stderr freely for progress or debugging: it passes straight through and
never affects submission.

An answer that has been accepted is remembered under `$XDG_STATE_HOME/aoc`, so
re-running a solved puzzle verifies locally instead of submitting again.

### Per-language commands

| Language | Scaffold | Build | Run |
| --- | --- | --- | --- |
| `rust` | `cargo init --bin` | `cargo build --release` | `cargo run --release` |
| `csharp` | `dotnet new console` | `dotnet build -c Release` | `dotnet run -c Release --no-build` |
| `java` | *(creates `Main.java`)* | `javac Main.java` | `java -cp . Main` |
| `python` | *(creates `main.py`)* | — | `python3 main.py` |

Your solution should read its input from `../input.txt`, relative to the
project directory — one input per day, shared across languages.

## Environment

| Variable | Effect |
| --- | --- |
| `AOC_SESSION` | Session cookie; overrides `cookie` in the config file. |
| `AOC_CONFIG_DIR` | Configuration directory; overrides the default location. |
| `XDG_CONFIG_HOME` | Used as `$XDG_CONFIG_HOME/aoc` when set. |
| `XDG_STATE_HOME` | Where accepted answers and the request throttle are cached. |
| `NO_COLOR` | Disables coloured output. |

The configuration directory is the first of `--config`'s parent directory,
`$AOC_CONFIG_DIR`, `$XDG_CONFIG_HOME/aoc`, or `~/.config/aoc`.

Data goes to stdout and diagnostics go to stderr, so `cd $(aoc path)` is safe.

## Automation etiquette

This tool follows the Advent of Code
[automation guidelines](https://www.reddit.com/r/adventofcode/wiki/faqs/automation).
Nothing here polls, scrapes or runs on a schedule — a request is made only when
you run `aoc` — and the requests that are made are governed as follows.

- **Every request identifies this tool**, naming the project, its version and
  an address to reach its author. The identification is baked into the one HTTP
  client `LiveClient` builds, so no request can go out without it.
- **Outbound calls are throttled** by `Throttle::acquire` in
  `src/aoc/throttle.rs`, which holds a minimum gap of five seconds between
  requests. It wraps the HTTP transport itself rather than the call sites, so
  every request passes through it — including the one the client makes on its
  own to check an answer against a part that turned out to be solved already.
  `LiveClient` in `src/aoc/live.rs` is the only code in the crate that contacts
  the site. The moment of the last request is recorded in the state directory,
  so the gap is honoured across separate invocations and not merely within a
  single run.
- **Inputs are cached after the initial download** by `App::ensure_input` in
  `src/app.rs`, which fetches a day's input only when `input.txt` is not
  already on disk. To retrieve a fresh copy of an input you suspect is
  corrupted, delete that file and run again; the replacement download is
  throttled like any other request.
- **Accepted answers are cached** by `FileCache` in `src/aoc/cache.rs`, so a
  part that was already solved is verified against the local record instead of
  being re-submitted on every run.

Please do not work around any of these.

## Development

```console
$ cargo test          # unit and end-to-end tests; no network needed
$ cargo clippy --all-targets
$ cargo fmt --all --check
$ cargo deny check
```

The crate is a library plus a thin binary. The clock, child processes, the
Advent of Code API and the terminal each sit behind a trait, so the whole tool
is testable without a network, a language toolchain or a real calendar.

Releases are handled by [release-plz](https://release-plz.dev): merging the
generated release pull request publishes to crates.io, tags `vX.Y.Z` and
attaches prebuilt binaries.

## License

[GPL-3.0](LICENSE)
