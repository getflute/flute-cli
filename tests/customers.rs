//! Binary-level integration tests for the `customers` subcommand group.
//!
//! All tests here are deterministic and require no network access, no OS
//! keychain, and no live credentials.  They exercise:
//!   1. `--help` output for each subcommand (flags documented correctly).
//!   2. Clap-level required-argument enforcement (missing args → non-zero exit).
//!   3. Runtime `--yes` guard enforcement for destructive operations (the guard
//!      fires **before** `build_client`, so no credentials are required).
//!
// NOTE: These binary tests are intentionally network- and keychain-free.  The
// ApiClient is wired to a profile-hardcoded URL, so wire/body correctness is
// covered by lib unit tests and wiremock tests.  Here we only assert on Clap
// argument parsing, help-text output, and the pre-credential --yes guards —
// all of which fire before any credential or network path is reached.  This
// mirrors the pattern established in tests/transactions.rs.

use assert_cmd::Command;
use predicates::prelude::*;

// ── Helper ───────────────────────────────────────────────────────────────────

fn flute() -> Command {
    Command::cargo_bin("flute").expect("binary must be compiled")
}

// ── customers create --help ───────────────────────────────────────────────────

/// `flute customers create --help` exits 0 and documents --first-name and --email.
#[test]
fn customers_create_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["customers", "create", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--first-name").and(predicate::str::contains("--email")));
}

// ── customers get --help ──────────────────────────────────────────────────────

/// `flute customers get --help` exits 0 and mentions the id argument.
#[test]
fn customers_get_help_exits_zero_and_mentions_id() {
    flute()
        .args(["customers", "get", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id"));
}

// ── customers list --help ─────────────────────────────────────────────────────

/// `flute customers list --help` exits 0 and documents --limit and --search.
#[test]
fn customers_list_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["customers", "list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--limit").and(predicate::str::contains("--search")));
}

// ── customers update --help ───────────────────────────────────────────────────

/// `flute customers update --help` exits 0 and documents the id argument.
#[test]
fn customers_update_help_exits_zero_and_mentions_id() {
    flute()
        .args(["customers", "update", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id"));
}

// ── customers delete --help ───────────────────────────────────────────────────

/// `flute customers delete --help` exits 0 and documents --yes.
#[test]
fn customers_delete_help_exits_zero_and_mentions_yes() {
    flute()
        .args(["customers", "delete", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--yes"));
}

// ── customers add-card --help ─────────────────────────────────────────────────

/// `flute customers add-card --help` exits 0 and documents --card and --exp.
#[test]
fn customers_add_card_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["customers", "add-card", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--card").and(predicate::str::contains("--exp")));
}

// ── customers add-ach --help ──────────────────────────────────────────────────

/// `flute customers add-ach --help` exits 0 and documents --routing and --account.
#[test]
fn customers_add_ach_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["customers", "add-ach", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--routing").and(predicate::str::contains("--account")));
}

// ── customers methods --help ──────────────────────────────────────────────────

/// `flute customers methods --help` exits 0 and mentions the customer_id argument.
#[test]
fn customers_methods_help_exits_zero_and_mentions_customer_id() {
    flute()
        .args(["customers", "methods", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("customer"));
}

// ── customers remove-method --help ────────────────────────────────────────────

/// `flute customers remove-method --help` exits 0 and documents --yes.
#[test]
fn customers_remove_method_help_exits_zero_and_mentions_yes() {
    flute()
        .args(["customers", "remove-method", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--yes"));
}

// ── Required-argument enforcement (Clap, no network) ─────────────────────────

/// `flute customers get` with no positional `id` must exit non-zero (Clap
/// required positional argument missing).
#[test]
fn customers_get_without_id_fails() {
    flute().args(["customers", "get"]).assert().failure();
}

// ── Runtime --yes guard (fires before build_client, no network) ───────────────

/// `flute customers delete cust-1` WITHOUT `--yes` must exit non-zero and
/// mention `--yes` in stderr.
///
/// The `--yes` guard fires **before** `build_client` in the dispatch path, so
/// this test is always deterministic — no credentials or network access needed.
#[test]
fn customers_delete_without_yes_fails_and_mentions_yes_flag() {
    flute()
        .args(["customers", "delete", "cust-1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"));
}

/// `flute customers remove-method cust-1 method-1` WITHOUT `--yes` must exit
/// non-zero and mention `--yes` in stderr.
///
/// The `--yes` guard fires **before** `build_client` in the dispatch path, so
/// this test is always deterministic — no credentials or network access needed.
#[test]
fn customers_remove_method_without_yes_fails_and_mentions_yes_flag() {
    flute()
        .args(["customers", "remove-method", "cust-1", "method-1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"));
}
