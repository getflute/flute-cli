//! Binary-level integration tests for the `tokens` subcommand group.
//!
//! All tests here are deterministic and require no network access, no OS
//! keychain, and no live credentials.  They exercise:
//!   1. `--help` output for each subcommand (flags documented correctly).
//!   2. Clap-level required-argument enforcement (missing args → non-zero exit).
//!   3. Runtime `--yes` guard enforcement for the revoke operation (the guard
//!      fires **before** `build_client`, so no credentials are required).
//!
// NOTE: These binary tests are intentionally network- and keychain-free.  The
// ApiClient is wired to a profile-hardcoded URL, so wire/body correctness is
// covered by lib unit tests and wiremock tests.  Here we only assert on Clap
// argument parsing, help-text output, and the pre-credential --yes guards —
// all of which fire before any credential or network path is reached.  This
// mirrors the pattern established in tests/customers.rs and tests/pos.rs.

use assert_cmd::Command;
use predicates::prelude::*;

// ── Helper ───────────────────────────────────────────────────────────────────

fn flute() -> Command {
    Command::cargo_bin("flute").expect("binary must be compiled")
}

// ── tokens create --help ──────────────────────────────────────────────────────

/// `flute tokens create --help` exits 0 and documents --merchant-id and --name.
#[test]
fn tokens_create_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["tokens", "create", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--merchant-id").and(predicate::str::contains("--name")));
}

// ── tokens list --help ────────────────────────────────────────────────────────

/// `flute tokens list --help` exits 0 and mentions --merchant-id (optional
/// filter).
#[test]
fn tokens_list_help_exits_zero_and_mentions_merchant_id() {
    flute()
        .args(["tokens", "list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--merchant-id"));
}

// ── tokens revoke --help ──────────────────────────────────────────────────────

/// `flute tokens revoke --help` exits 0 and documents --client-id, --merchant-id and --yes.
#[test]
fn tokens_revoke_help_exits_zero_and_mentions_client_id_merchant_id_and_yes() {
    flute()
        .args(["tokens", "revoke", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--client-id")
                .and(predicate::str::contains("--merchant-id"))
                .and(predicate::str::contains("--yes")),
        );
}

// ── Required-argument enforcement (Clap, no network) ─────────────────────────

/// `flute tokens create` with no required flags must exit non-zero (Clap
/// required flag missing — both --merchant-id and --name are required).
#[test]
fn tokens_create_without_required_flags_fails() {
    flute().args(["tokens", "create"]).assert().failure();
}

/// `flute tokens create --merchant-id <id>` without --name must exit non-zero
/// (--name is required).
#[test]
fn tokens_create_without_name_fails() {
    flute()
        .args(["tokens", "create", "--merchant-id", "merchant-abc-001"])
        .assert()
        .failure();
}

/// `flute tokens revoke` with no --client-id must exit non-zero (--client-id
/// is required by Clap).
#[test]
fn tokens_revoke_without_client_id_fails() {
    flute().args(["tokens", "revoke"]).assert().failure();
}

/// `flute tokens revoke --client-id <dummy>` without `--merchant-id` must exit
/// non-zero (--merchant-id is required by Clap).
#[test]
fn tokens_revoke_without_merchant_id_fails() {
    flute()
        .args([
            "tokens",
            "revoke",
            "--client-id",
            "00000000-0000-0000-0000-000000000000",
        ])
        .assert()
        .failure();
}

// ── Runtime --yes guard (fires before build_client, no network) ───────────────

/// `flute tokens revoke --client-id <dummy> --merchant-id <dummy>` WITHOUT `--yes` must exit
/// non-zero and mention `--yes` in stderr.
///
/// The `--yes` guard fires **before** `build_client` in the dispatch path, so
/// this test is always deterministic — no credentials or network access needed.
#[test]
fn tokens_revoke_without_yes_fails_and_mentions_yes_flag() {
    flute()
        .args([
            "tokens",
            "revoke",
            "--client-id",
            "00000000-0000-0000-0000-000000000000",
            "--merchant-id",
            "11111111-0000-0000-0000-000000000000",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"));
}
