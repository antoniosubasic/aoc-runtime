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
| `cookie` | no | none | Session cookie, or a `COOKIE` file beside this one. Without it, `aoc` never contacts adventofcode.com. |
| `editor` | no | `code` | Command run by `aoc code`. |

To find your session cookie, open adventofcode.com while logged in, and copy
the value of the `session` cookie from your browser's developer tools. It is a
credential: treat it like a password. If you would rather not keep it in a
file, set `AOC_SESSION` instead — it takes precedence over the config file.

To keep the secret out of `config.yaml` — so the config itself can live in a
dotfiles repository — put the cookie on its own in `~/.config/aoc/COOKIE`
instead. It is read only when neither `AOC_SESSION` nor `cookie` supplies one,
and its contents are taken raw, with surrounding whitespace trimmed. A `COOKIE`
file that exists but cannot be read is a warning rather than an error, so it
never stops a command that needs no cookie.

### Template placeholders

| Placeholder | Expands to |
| --- | --- |
| `{{year}}` | The four-digit year, e.g. `2024` |
| `{{day}}` | The day, unpadded, e.g. `7` |
| `{{pad day}}` | The day, zero-padded to two digits, e.g. `07` |
| `{{language}}` | `rust`, `csharp`, `java`, `python`, `javascript`, `go`, `c`, `cpp`, `ruby` or `bash` |

Whitespace inside the braces is insignificant, so `{{pad day}}` and
`{{ pad day }}` are the same placeholder. `{{year}}` and a day placeholder are
required; `{{language}}` is optional, though without it `aoc` cannot tell your
languages apart when reading the current directory.

The template is used in both directions. It builds the project path, and it is
also compiled into a pattern that recovers the year, day and language *from*
the directory you are in — which is why `aoc` on its own usually does the right
thing.

### Base files

If `~/.config/aoc/base/<language>` exists, `aoc init` copies it over the
generated entry point, so every new project starts from your own boilerplate.
The file is named after the language and carries no extension.

| Language | Base file | Copied to |
| --- | --- | --- |
| `rust` | `base/rust` | `src/main.rs` |
| `csharp` | `base/csharp` | `Program.cs` |
| `java` | `base/java` | `Main.java` |
| `python` | `base/python` | `main.py` |
| `javascript` | `base/javascript` | `main.js` |
| `go` | `base/go` | `main.go` |
| `c` | `base/c` | `main.c` |
| `cpp` | `base/cpp` | `main.cpp` |
| `ruby` | `base/ruby` | `main.rb` |
| `bash` | `base/bash` | `main.sh` |

## Usage

```
aoc [OPTIONS] [MODE]...
```

| Mode | What it does |
| --- | --- |
| `run` *(default)* | Build the solution, run it, submit the answers it prints |
| `init` | Create the project directory and scaffold a solution |
| `path` | Print the project directory |
| `code` | Open the project directory in your editor |
| `url` | Print the puzzle URL |
| `open` | Open the puzzle in your default browser |

| Option | Description |
| --- | --- |
| `-y, --year <YEAR>` | Puzzle year. Defaults to the most recent event. |
| `-d, --day <DAY>` | Puzzle day, `1`–`25` (`1`–`12` from 2025 on). Defaults to today during December, otherwise `1`. |
| `-l, --language <LANGUAGE>` | `rust`, `csharp`, `java`, `python`, `javascript`, `go`, `c`, `cpp`, `ruby` or `bash`. |
| `--no-submit` | Run the solution but do not submit its answers. |
| `--config <FILE>` | Use a specific config file. |
| `-h, --help` / `-V, --version` | |

Neither `url` nor `open` needs a language.

Several modes run one after another, in the order given:

```console
$ aoc init code          # scaffold the project, then open it in your editor
$ aoc init run           # scaffold it, then build, run and submit
$ aoc open init code     # read the puzzle, scaffold it, open your editor
```

They share the same year, day and language, and nothing is carried from one to
the next. If one fails the ones after it do not run, and a mode missing its
language is caught before any of them starts.

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
re-running a solved puzzle verifies locally instead of submitting again. Puzzle
inputs and build output are kept there too — see [Inputs](#inputs) and
[Build output](#build-output).

### Per-language commands

A compiled language is built optimized once and then executed directly, with no
build tool left in the loop.

| Language | Scaffold | Build | Run |
| --- | --- | --- | --- |
| `rust` | `cargo init --bin` | `cargo build --release` | the binary in `target/release` |
| `csharp` | `dotnet new console` | `dotnet build -c Release` | the launcher in `bin/Release` |
| `java` | *(creates `Main.java`)* | `javac -d …` | `java -cp … Main` |
| `go` | `go mod init` | `go build` | the built binary |
| `c` | *(creates `main.c`)* | `cc -O2` | the built binary |
| `cpp` | *(creates `main.cpp`)* | `c++ -O2` | the built binary |
| `python` | *(creates `main.py`)* | — | `python3 main.py` |
| `javascript` | *(creates `main.js`)* | — | `node main.js` |
| `ruby` | *(creates `main.rb`)* | — | `ruby main.rb` |
| `bash` | *(creates `main.sh`)* | — | `bash main.sh` |

`c` and `cpp` go through `cc` and `c++`, so they use whichever compiler your
system installed. `python` falls back to `python` where there is no `python3`,
and `javascript` to `nodejs`. `go mod init` writes only a `go.mod`, so a `go`
project starts from your `base/go` file — see [Base files](#base-files).

Nothing a build produces is written into your project — see
[Build output](#build-output). Whatever it takes to run it, a solution is always
executed from its project directory, so it should read its input from
`../input.txt` — one input per day, shared across languages.

### Inputs

An input is personal, permanent and unchanging, so `aoc` downloads each one
exactly once and keeps it under `$XDG_STATE_HOME/aoc/inputs`. The `input.txt`
beside your project is a symbolic link into that directory, written whenever a
project is missing one.

The practical effect is that the download survives the solutions tree. Delete a
project, re-scaffold a day, move `~/projects/aoc` somewhere else or clone it
onto a second machine, and the next `aoc run` links the input back rather than
asking the site for it again. (On Windows, where creating a link needs developer
mode, `aoc` copies the cached input instead — the download still happens only
once.)

To force a fresh download, delete the cached copy and run again; for 2024 day 7
that is `$XDG_STATE_HOME/aoc/inputs/2024-07.txt`. Deleting only the project's
`input.txt` re-links it and contacts nothing.

### Build output

Compiling a solution should not leave anything lying around your solutions tree.
Where the output goes depends on whether the language already has a directory of
its own for it.

`java`, `go`, `c` and `cpp` are compiled by hand, so their output goes to
`$XDG_STATE_HOME/aoc/builds/<year>-<day>/<language>` — for 2024 day 7 in C, the
binary is `$XDG_STATE_HOME/aoc/builds/2024-07/c/bin`. Nothing is written beside
your sources.

`rust` and `csharp` build where their own tooling expects, into `target/` and
`bin/` inside the project. Redirecting those elsewhere buys nothing: your editor
builds into the conventional directory regardless, so the project would end up
with one anyway plus a second copy in the state directory. Cargo writes a
`.gitignore` covering `target/` when it scaffolds; for C# add `bin/` and `obj/`
to your solutions tree's ignore file.

Either way the solution is executed directly, with the build tool used only as a
fallback. Build output is rebuilt on every `aoc run` and is safe to delete at any
time — deleting a project reclaims its own, though nothing prunes the state
directory for you.

## Environment

| Variable | Effect |
| --- | --- |
| `AOC_SESSION` | Session cookie; overrides `cookie` in the config file and the `COOKIE` file. |
| `AOC_CONFIG_DIR` | Configuration directory; overrides the default location. |
| `XDG_CONFIG_HOME` | Used as `$XDG_CONFIG_HOME/aoc` when set. |
| `XDG_STATE_HOME` | Where puzzle inputs, compiled binaries, accepted answers and the request throttle are kept. |
| `NO_COLOR` | Disables coloured output. |

The configuration directory is the first of `--config`'s parent directory,
`$AOC_CONFIG_DIR`, `$XDG_CONFIG_HOME/aoc`, or `~/.config/aoc`. The `base`
directory and the `COOKIE` file are looked for there too. A relative `--config`
path is resolved against the current directory, so `--config config.yaml` makes
the directory you are standing in the configuration directory.

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
- **Every input is downloaded at most once** by `App::ensure_input` in
  `src/app.rs`, which asks the site for a day only when `InputStore` in
  `src/aoc/input.rs` does not already hold it. The copy lives in the state
  directory and each project's `input.txt` links to it, so no amount of
  deleting, moving or re-scaffolding projects can cost a second download. To
  retrieve a fresh copy of an input you suspect is corrupted, delete the cached
  file itself; the replacement download is throttled like any other request.
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
