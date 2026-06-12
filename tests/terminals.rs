//! Binary-level integration tests for the `terminals` subcommand group.
//!
//! All tests here are deterministic and require no network access, no OS
//! keychain, and no live credentials.  They exercise:
//!   1. `--help` output for each subcommand (flags documented correctly).
//!   2. Clap-level required-argument enforcement (missing args → non-zero exit).
//!
// NOTE: These binary tests are intentionally network- and keychain-free.  The
// ApiClient is wired to a profile-hardcoded URL, so wire/body correctness is
// covered by lib unit tests and wiremock tests.  Here we only assert on Clap
// argument parsing and help-text output — all of which fire before any
// credential or network path is reached.  This mirrors the pattern established
// in tests/transactions.rs and tests/customers.rs.

use assert_cmd::Command;
use predicates::prelude::*;

// ── Helper ───────────────────────────────────────────────────────────────────

fn flute() -> Command {
    Command::cargo_bin("flute").expect("binary must be compiled")
}

// ── terminals list --help ─────────────────────────────────────────────────────

/// `flute terminals list --help` exits 0 and documents --limit.
#[test]
fn terminals_list_help_exits_zero_and_mentions_limit() {
    flute()
        .args(["terminals", "list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--limit"));
}

// ── terminals status --help ───────────────────────────────────────────────────

/// `flute terminals status --help` exits 0 and mentions the positional id argument.
#[test]
fn terminals_status_help_exits_zero_and_mentions_id() {
    flute()
        .args(["terminals", "status", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id"));
}

// ── Required-argument enforcement (Clap, no network) ─────────────────────────

/// `flute terminals status` with no positional `id` must exit non-zero (Clap
/// required positional argument missing).
#[test]
fn terminals_status_without_id_fails() {
    flute().args(["terminals", "status"]).assert().failure();
}
