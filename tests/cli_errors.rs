//! Exit-code / error-output contract (ARISE-4706 review, issue 1).
//!
//! Client-side problems — bad input AND clap usage/parse errors — are
//! validation errors → **exit 3** (not clap's default 2, which collides with
//! the auth code). Under `--output json` a usage error is still emitted as a
//! structured `{kind:"client"}` envelope on **stdout**. `--help`/`--version`
//! remain exit 0.

use assert_cmd::Command;
use predicates::prelude::*;

fn flute() -> Command {
    Command::cargo_bin("flute").expect("binary must be compiled")
}

#[test]
fn missing_required_arg_exits_3_stderr() {
    flute()
        .args(["transactions", "sale"])
        .assert()
        .code(3)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("--amount"));
}

#[test]
fn missing_required_arg_json_emits_client_envelope_on_stdout_exit_3() {
    flute()
        .args(["--output", "json", "transactions", "sale"])
        .assert()
        .code(3)
        .stdout(
            predicate::str::contains("\"kind\": \"client\"")
                .and(predicate::str::contains("--amount")),
        );
}

#[test]
fn client_side_validation_bad_amount_exits_3() {
    flute()
        .args([
            "--output",
            "json",
            "transactions",
            "sale",
            "--amount",
            "not-a-number",
            "--card",
            "4111111111111111",
            "--exp",
            "12/27",
            "--cvv",
            "123",
        ])
        .assert()
        .code(3)
        .stdout(predicate::str::contains("\"kind\": \"client\""));
}

#[test]
fn unknown_subcommand_exits_3() {
    flute().arg("definitely-not-a-command").assert().code(3);
}

#[test]
fn help_and_version_exit_zero() {
    flute().arg("--help").assert().success();
    flute().arg("--version").assert().success();
}
