//! End-to-end tests that run the real `aoc` binary.
//!
//! These cover the command line contract: exit codes, what lands on stdout
//! versus stderr, and the modes that need no language toolchain. Anything
//! requiring `cargo`, `dotnet` or `javac` is covered by unit tests against a
//! fake command runner instead.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod support;

use predicates::prelude::*;
use std::fs;
use support::{Fixture, yaml_string};

#[test]
fn help_works_without_a_config_file() {
    let fixture = Fixture::new();

    fixture
        .command_without_config()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage: aoc [OPTIONS] [MODE]"))
        .stdout(predicate::str::contains("- run:"))
        .stdout(predicate::str::contains("- init:"))
        .stdout(predicate::str::contains("- path:"))
        .stdout(predicate::str::contains("- code:"))
        .stdout(predicate::str::contains("- url:"))
        .stdout(predicate::str::contains("Puzzle year"))
        .stdout(predicate::str::contains("Solution language"));
}

#[test]
fn version_works_without_a_config_file() {
    let fixture = Fixture::new();

    fixture
        .command_without_config()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("aoc "));
}

#[test]
fn a_missing_config_file_explains_where_it_should_be() {
    let fixture = Fixture::new();

    fixture
        .command_without_config()
        .args(["url", "-y", "2024", "-d", "5"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no config file at"))
        .stderr(predicate::str::contains("template_path"));
}

#[test]
fn url_prints_the_puzzle_link_and_needs_no_language() {
    let fixture = Fixture::new();

    fixture
        .command()
        .args(["url", "-y", "2024", "-d", "5"])
        .assert()
        .success()
        .stdout("https://adventofcode.com/2024/day/5\n");
}

#[test]
fn a_day_past_the_end_of_a_shortened_event_is_rejected() {
    let fixture = Fixture::new();

    fixture
        .command()
        .args(["url", "-y", "2025", "-d", "20"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("2025 has no day 20"));

    fixture
        .command()
        .args(["url", "-y", "2025", "-d", "12"])
        .assert()
        .success()
        .stdout("https://adventofcode.com/2025/day/12\n");
}

#[test]
fn path_prints_the_project_directory() {
    let fixture = Fixture::new();

    fixture
        .command()
        .args(["path", "-y", "2024", "-d", "5", "-l", "rust"])
        .assert()
        .success()
        .stdout(format!(
            "{}\n",
            fixture.project("2024/day05/rust").display()
        ));
}

#[test]
fn path_recovers_parameters_from_the_working_directory() {
    let fixture = Fixture::new();
    let project = fixture.project("2019/day03/java");

    fixture
        .command_in(&project)
        .arg("path")
        .assert()
        .success()
        .stdout(format!("{}\n", project.display()));
}

#[test]
fn two_digit_days_are_recovered_correctly() {
    let fixture = Fixture::new();

    for day in [5, 9, 10, 15, 25] {
        let project = fixture.project(&format!("2024/day{day:02}/python"));

        fixture
            .command_in(&project)
            .arg("path")
            .assert()
            .success()
            .stdout(format!("{}\n", project.display()));
    }
}

#[test]
fn a_directory_deeper_than_the_project_still_resolves() {
    let fixture = Fixture::new();
    let project = fixture.project("2024/day07/rust");

    fixture
        .command_in(&project.join("src"))
        .arg("path")
        .assert()
        .success()
        .stdout(format!("{}\n", project.display()));
}

#[test]
fn modes_other_than_url_require_a_language() {
    let fixture = Fixture::new();

    for mode in ["run", "init", "path", "code"] {
        fixture
            .command()
            .args([mode, "-y", "2024", "-d", "5"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("a language is required"))
            .stderr(predicate::str::contains(mode));
    }
}

#[test]
fn init_scaffolds_an_interpreted_project() {
    let fixture = Fixture::new();

    fixture
        .command()
        .args(["init", "-y", "2024", "-d", "5", "-l", "python"])
        .assert()
        .success();

    let entry = fixture.project("2024/day05/python/main.py");
    assert!(entry.is_file(), "expected {} to exist", entry.display());
    assert_eq!(fs::read_to_string(&entry).expect("entry file"), "");
}

#[test]
fn init_copies_the_base_file_over_the_entry_point() {
    let fixture = Fixture::new();
    fixture.write_base_file("base.py", "import sys\n\nprint('hello')\n");

    fixture
        .command()
        .args(["init", "-y", "2024", "-d", "5", "-l", "python"])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(fixture.project("2024/day05/python/main.py")).expect("entry file"),
        "import sys\n\nprint('hello')\n"
    );
}

#[test]
fn init_refuses_to_overwrite_an_existing_project() {
    let fixture = Fixture::new();
    let args = ["init", "-y", "2024", "-d", "5", "-l", "java"];

    fixture.command().args(args).assert().success();
    fixture
        .command()
        .args(args)
        .assert()
        .failure()
        .stderr(predicate::str::contains("project already exists"));
}

#[test]
fn run_refuses_to_run_a_project_that_does_not_exist() {
    let fixture = Fixture::new();

    fixture
        .command()
        .args(["run", "-y", "2024", "-d", "5", "-l", "rust"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("project does not exist"))
        .stderr(predicate::str::contains("aoc init"));
}

#[test]
fn invalid_arguments_are_usage_errors() {
    let fixture = Fixture::new();

    for args in [
        ["-d", "26"].as_slice(),
        ["-d", "0"].as_slice(),
        ["-y", "2014"].as_slice(),
        ["-l", "cobol"].as_slice(),
        ["compile"].as_slice(),
    ] {
        fixture
            .command()
            .args(args)
            .assert()
            .code(2)
            .stderr(predicate::str::contains("error:"));
    }
}

#[test]
fn an_invalid_template_is_reported_against_the_config_file() {
    let fixture = Fixture::new();
    fixture.write_config("template_path: \"/aoc/{{year}}/{{month}}\"\n");

    fixture
        .command()
        .args(["url", "-y", "2024", "-d", "5"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid `template_path`"))
        .stderr(predicate::str::contains("month"));
}

#[test]
fn a_template_missing_the_day_is_rejected_at_load_time() {
    let fixture = Fixture::new();
    fixture.write_config("template_path: \"/aoc/{{year}}/{{language}}\"\n");

    fixture
        .command()
        .args(["path", "-l", "rust"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "missing the `{{day}}` placeholder",
        ));
}

#[test]
fn an_unknown_config_key_warns_but_still_runs() {
    let fixture = Fixture::new();
    fixture.write_config(&format!(
        "template_path: {}\ncookies: typo\n",
        yaml_string(fixture.template())
    ));

    fixture
        .command()
        .args(["url", "-y", "2024", "-d", "5"])
        .assert()
        .success()
        .stdout("https://adventofcode.com/2024/day/5\n")
        .stderr(predicate::str::contains("unknown key `cookies`"));
}

#[test]
fn diagnostics_stay_off_stdout() {
    let fixture = Fixture::new();
    fixture.write_config(&format!(
        "template_path: {}\nnonsense: 1\n",
        yaml_string(fixture.template())
    ));

    let output = fixture
        .command()
        .args(["path", "-y", "2024", "-d", "5", "-l", "rust"])
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).expect("utf-8 stdout");
    assert_eq!(
        stdout.trim_end(),
        fixture.project("2024/day05/rust").to_string_lossy()
    );
}

#[test]
fn an_explicit_config_file_overrides_the_default_location() {
    let fixture = Fixture::new();
    let elsewhere = fixture.root().join("elsewhere");
    fs::create_dir_all(&elsewhere).expect("create dir");
    fs::write(
        elsewhere.join("other.yaml"),
        "template_path: \"/somewhere/{{year}}/day{{day}}/{{language}}\"\n",
    )
    .expect("write config");

    fixture
        .command()
        .args([
            "--config",
            &elsewhere.join("other.yaml").to_string_lossy(),
            "path",
            "-y",
            "2024",
            "-d",
            "5",
            "-l",
            "rust",
        ])
        .assert()
        .success()
        .stdout("/somewhere/2024/day5/rust\n");
}

#[test]
fn no_ansi_escapes_when_colour_is_disabled() {
    let fixture = Fixture::new();

    let output = fixture
        .command()
        .args(["url", "-y", "2024", "-d", "5"])
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).expect("utf-8 stdout");
    assert!(
        !stdout.contains('\u{1b}'),
        "unexpected escape in {stdout:?}"
    );
}
