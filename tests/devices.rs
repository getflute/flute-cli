//! Binary-level integration tests for the `devices` subcommand group.
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

// ── devices list --help ───────────────────────────────────────────────────────

/// `flute devices list --help` exits 0.
#[test]
fn devices_list_help_exits_zero() {
    flute()
        .args(["devices", "list", "--help"])
        .assert()
        .success();
}

// ── devices get --help ────────────────────────────────────────────────────────

/// `flute devices get --help` exits 0 and mentions the positional id argument.
#[test]
fn devices_get_help_exits_zero_and_mentions_id() {
    flute()
        .args(["devices", "get", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id"));
}

// ── devices register --help ───────────────────────────────────────────────────

/// `flute devices register --help` exits 0 and mentions both the positional id
/// argument and the --name flag.
#[test]
fn devices_register_help_exits_zero_and_mentions_id_and_name() {
    flute()
        .args(["devices", "register", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id").and(predicate::str::contains("--name")));
}

// ── devices ttp-jwt --help ────────────────────────────────────────────────────

/// `flute devices ttp-jwt --help` exits 0 and mentions --device-id.
#[test]
fn devices_ttp_jwt_help_exits_zero_and_mentions_device_id() {
    flute()
        .args(["devices", "ttp-jwt", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--device-id"));
}

// ── devices ttp-activate --help ───────────────────────────────────────────────

/// `flute devices ttp-activate --help` exits 0 and mentions the positional id argument.
#[test]
fn devices_ttp_activate_help_exits_zero_and_mentions_id() {
    flute()
        .args(["devices", "ttp-activate", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id"));
}

// ── Required-argument enforcement (Clap, no network) ─────────────────────────

/// `flute devices get` with no positional `id` must exit non-zero (Clap
/// required positional argument missing).
#[test]
fn devices_get_without_id_fails() {
    flute().args(["devices", "get"]).assert().failure();
}

/// `flute devices ttp-jwt` with no `--device-id` must exit non-zero (Clap
/// required flag missing).
#[test]
fn devices_ttp_jwt_without_device_id_fails() {
    flute().args(["devices", "ttp-jwt"]).assert().failure();
}

/// `flute devices register` with no positional `id` must exit non-zero (Clap
/// required positional argument missing).
#[test]
fn devices_register_without_id_fails() {
    flute().args(["devices", "register"]).assert().failure();
}
