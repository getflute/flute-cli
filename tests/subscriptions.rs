//! Binary-level integration tests for the `subscriptions` subcommand group.
//!
//! All tests here are deterministic and require no network access, no OS
//! keychain, and no live credentials.  They exercise:
//!   1. `--help` output for each subcommand (flags documented correctly).
//!   2. Clap-level required-argument enforcement (missing args → non-zero exit).
//!   3. Runtime `--yes` guard enforcement for the terminate operation (the
//!      guard fires **before** `build_client`, so no credentials are required).
//!   4. Alias acceptance: `--interval monthly` must not produce a Clap "invalid
//!      value" error (the alias is wired in the `Interval` ValueEnum).
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

// ── subscriptions create --help ───────────────────────────────────────────────

/// `flute subscriptions create --help` exits 0 and documents --customer-id,
/// --payment-method-id, --amount, --number-of-payments, and --interval.
#[test]
fn subscriptions_create_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["subscriptions", "create", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--customer-id")
                .and(predicate::str::contains("--payment-method-id"))
                .and(predicate::str::contains("--amount"))
                .and(predicate::str::contains("--number-of-payments"))
                .and(predicate::str::contains("--interval")),
        );
}

// ── subscriptions get --help ──────────────────────────────────────────────────

/// `flute subscriptions get --help` exits 0 and mentions the positional id
/// argument.
#[test]
fn subscriptions_get_help_exits_zero_and_mentions_id() {
    flute()
        .args(["subscriptions", "get", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id"));
}

// ── subscriptions list --help ─────────────────────────────────────────────────

/// `flute subscriptions list --help` exits 0 and documents --limit and
/// --customer-id.
#[test]
fn subscriptions_list_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["subscriptions", "list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--limit").and(predicate::str::contains("--customer-id")));
}

// ── subscriptions payments --help ─────────────────────────────────────────────

/// `flute subscriptions payments --help` exits 0 and mentions the positional
/// id argument.
#[test]
fn subscriptions_payments_help_exits_zero_and_mentions_id() {
    flute()
        .args(["subscriptions", "payments", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id"));
}

// ── subscriptions terminate --help ───────────────────────────────────────────

/// `flute subscriptions terminate --help` exits 0 and documents --yes and the
/// positional id argument.
#[test]
fn subscriptions_terminate_help_exits_zero_and_mentions_yes_and_id() {
    flute()
        .args(["subscriptions", "terminate", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--yes").and(predicate::str::contains("id")));
}

// ── Required-argument enforcement (Clap, no network) ─────────────────────────

/// `flute subscriptions create` with NO required flags must exit non-zero
/// (Clap required flag missing: --customer-id, --payment-method-id, --amount,
/// and --number-of-payments are all required).
#[test]
fn subscriptions_create_without_required_flags_fails() {
    flute().args(["subscriptions", "create"]).assert().failure();
}

// ── Runtime --yes guard (fires before build_client, no network) ───────────────

/// `flute subscriptions terminate <dummy>` WITHOUT `--yes` must exit non-zero
/// and mention `--yes` in stderr.
///
/// The `--yes` guard fires **before** `build_client` in the dispatch path, so
/// this test is always deterministic — no credentials or network access needed.
#[test]
fn subscriptions_terminate_without_yes_fails_and_mentions_yes_flag() {
    flute()
        .args([
            "subscriptions",
            "terminate",
            "00000000-0000-0000-0000-000000000000",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"));
}

// ── Interval alias: `monthly` must not produce a Clap "invalid value" error ──

/// `flute subscriptions create` with all required flags and `--interval monthly`
/// must NOT fail with a Clap "invalid value" / "possible values" error.
///
/// Without credentials the command will fail at auth/network, but the failure
/// must be an auth error — not a Clap parse error about the interval value.
/// This verifies that the `monthly` alias is correctly wired on the `Interval`
/// ValueEnum (`#[value(name = "month", alias = "monthly")]`).
///
// NOTE: Binary tests avoid network/keychain.  When no credentials are present,
// the binary exits with an auth error (not a Clap error) — we simply assert
// that stderr does NOT contain "invalid value" or "possible values", confirming
// `--interval monthly` is accepted by Clap regardless of the subsequent
// auth/network outcome.
#[test]
fn subscriptions_create_interval_monthly_is_not_a_clap_error() {
    let output = flute()
        .args([
            "subscriptions",
            "create",
            "--customer-id",
            "x",
            "--payment-method-id",
            "y",
            "--amount",
            "1.00",
            "--number-of-payments",
            "1",
            "--interval",
            "monthly",
        ])
        .output()
        .expect("failed to run binary");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // `monthly` is a registered alias — Clap must NOT reject it.
    assert!(
        !stderr.contains("invalid value"),
        "stderr must not contain 'invalid value'; got: {stderr}"
    );
    assert!(
        !stderr.contains("possible values"),
        "stderr must not contain 'possible values'; got: {stderr}"
    );
}
