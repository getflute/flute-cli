//! Binary-level integration tests for the `pos` subcommand group.
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
// credential or network path is reached.  The --wait poll loop is unit-tested
// with tokio::time in src/cli/pos.rs.  This mirrors the pattern established
// in tests/transactions.rs and tests/customers.rs.

use assert_cmd::Command;
use predicates::prelude::*;

// ── Helper ───────────────────────────────────────────────────────────────────

fn flute() -> Command {
    Command::cargo_bin("flute").expect("binary must be compiled")
}

// ── pos create --help ─────────────────────────────────────────────────────────

/// `flute pos create --help` exits 0 and documents --terminal-id, --wait,
/// --wait-timeout, --amount, and --transaction-type.
#[test]
fn pos_create_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["pos", "create", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--terminal-id")
                .and(predicate::str::contains("--wait"))
                .and(predicate::str::contains("--wait-timeout"))
                .and(predicate::str::contains("--amount"))
                .and(predicate::str::contains("--transaction-type")),
        );
}

// ── pos get --help ────────────────────────────────────────────────────────────

/// `flute pos get --help` exits 0 and mentions the positional id argument.
#[test]
fn pos_get_help_exits_zero_and_mentions_id() {
    flute()
        .args(["pos", "get", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id"));
}

// ── pos list --help ───────────────────────────────────────────────────────────

/// `flute pos list --help` exits 0 and documents --terminal-id and --limit.
#[test]
fn pos_list_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["pos", "list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--terminal-id").and(predicate::str::contains("--limit")));
}

// ── pos cancel --help ─────────────────────────────────────────────────────────

/// `flute pos cancel --help` exits 0 and mentions the positional id argument.
#[test]
fn pos_cancel_help_exits_zero_and_mentions_id() {
    flute()
        .args(["pos", "cancel", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id"));
}

// ── Required-argument enforcement (Clap, no network) ─────────────────────────

/// `flute pos create` with no `--terminal-id` must exit non-zero (Clap
/// required flag missing).
#[test]
fn pos_create_without_terminal_id_fails() {
    flute().args(["pos", "create"]).assert().failure();
}

/// `flute pos get` with no positional `id` must exit non-zero (Clap
/// required positional argument missing).
#[test]
fn pos_get_without_id_fails() {
    flute().args(["pos", "get"]).assert().failure();
}

/// `flute pos cancel` with no positional `id` must exit non-zero (Clap
/// required positional argument missing).
#[test]
fn pos_cancel_without_id_fails() {
    flute().args(["pos", "cancel"]).assert().failure();
}

/// `flute pos create --terminal-id <id>` without `--pos-device-id` must exit
/// non-zero — the API rejects creates without a device id.
#[test]
fn pos_create_without_pos_device_id_fails() {
    flute()
        .args([
            "pos",
            "create",
            "--terminal-id",
            "term-abc",
            "--reference-id",
            "ref-123",
        ])
        .assert()
        .failure();
}

/// `flute pos create --terminal-id <id>` without `--reference-id` must exit
/// non-zero — the API rejects creates without a reference id.
#[test]
fn pos_create_without_reference_id_fails() {
    flute()
        .args([
            "pos",
            "create",
            "--terminal-id",
            "term-abc",
            "--pos-device-id",
            "dev-001",
        ])
        .assert()
        .failure();
}

/// `flute pos create --help` documents both --pos-device-id and --reference-id.
#[test]
fn pos_create_help_mentions_pos_device_id_and_reference_id() {
    flute()
        .args(["pos", "create", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--pos-device-id")
                .and(predicate::str::contains("--reference-id")),
        );
}
