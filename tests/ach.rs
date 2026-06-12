//! Binary-level integration tests for the `ach` subcommand group.
//!
//! All tests here are deterministic and require no network access, no OS
//! keychain, and no live credentials.  They exercise:
//!   1. `--help` output for each subcommand (flags documented correctly).
//!   2. Clap-level required-argument enforcement (missing args → non-zero exit).
//!
// NOTE: These binary tests are intentionally network- and keychain-free.  The
// ApiClient is wired to a profile-hardcoded URL, so wire/body correctness is
// covered by lib unit tests and wiremock tests.  Here we only assert on Clap
// argument parsing and help-text output — both of which fire before any
// credential or network path is reached.  This mirrors the pattern established
// in tests/transactions.rs.

use assert_cmd::Command;
use predicates::prelude::*;

// ── Helper ───────────────────────────────────────────────────────────────────

fn flute() -> Command {
    Command::cargo_bin("flute").expect("binary must be compiled")
}

// ── ach debit --help ──────────────────────────────────────────────────────────

/// `flute ach debit --help` exits 0 and documents the key flags.
#[test]
fn ach_debit_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["ach", "debit", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--payment-processor-id")
                .and(predicate::str::contains("--routing"))
                .and(predicate::str::contains("--account"))
                .and(predicate::str::contains("--sec-code")),
        );
}

// ── ach credit --help ─────────────────────────────────────────────────────────

/// `flute ach credit --help` exits 0 and documents the key flags.
#[test]
fn ach_credit_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["ach", "credit", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--payment-processor-id")
                .and(predicate::str::contains("--routing"))
                .and(predicate::str::contains("--account"))
                .and(predicate::str::contains("--sec-code")),
        );
}

// ── ach void --help ───────────────────────────────────────────────────────────

/// `flute ach void --help` exits 0 and documents the positional id argument.
#[test]
fn ach_void_help_exits_zero_and_mentions_id() {
    flute()
        .args(["ach", "void", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id"));
}

// ── ach refund --help ─────────────────────────────────────────────────────────

/// `flute ach refund --help` exits 0 and documents the positional id argument.
#[test]
fn ach_refund_help_exits_zero_and_mentions_id() {
    flute()
        .args(["ach", "refund", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id"));
}

// ── Required-argument enforcement (Clap, no network) ─────────────────────────

/// `flute ach debit` with no `--amount` and no `--payment-processor-id` must
/// exit non-zero (Clap required-argument error) and mention the missing arg.
///
/// This assertion fires entirely within the Clap argument parser — before any
/// credential or network path is reached — so it is always deterministic.
#[test]
fn ach_debit_without_required_args_fails_with_usage_error() {
    flute().args(["ach", "debit"]).assert().failure().stderr(
        predicate::str::contains("amount")
            .or(predicate::str::contains("payment-processor-id"))
            .or(predicate::str::contains("required")),
    );
}

/// `flute ach credit` with no `--amount` and no `--payment-processor-id` must
/// exit non-zero (Clap required-argument error).
#[test]
fn ach_credit_without_required_args_fails_with_usage_error() {
    flute().args(["ach", "credit"]).assert().failure().stderr(
        predicate::str::contains("amount")
            .or(predicate::str::contains("payment-processor-id"))
            .or(predicate::str::contains("required")),
    );
}

/// `flute ach void` with no positional `id` must exit non-zero (Clap required
/// positional argument missing).
#[test]
fn ach_void_without_id_fails() {
    flute().args(["ach", "void"]).assert().failure();
}

/// `flute ach refund` with no positional `id` must exit non-zero.
#[test]
fn ach_refund_without_id_fails() {
    flute().args(["ach", "refund"]).assert().failure();
}
