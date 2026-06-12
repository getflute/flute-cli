//! Binary-level integration tests for the `transactions` subcommand group.
//!
//! All tests here are deterministic and require no network access, no OS
//! keychain, and no live credentials.  They exercise:
//!   1. `--help` output for each subcommand (flags documented correctly).
//!   2. Clap-level required-argument enforcement (missing args → non-zero exit).
//!
//! NOTE on amount-content validation:
//! Decimal-amount parsing (e.g. "notanumber") is handled inside `dispatch_transactions`
//! *after* `build_client` resolves credentials from the OS keychain.  On a machine
//! with no stored credentials `build_client` short-circuits with an auth error
//! (exit 2) before the amount parser ever runs, making a content-validation test
//! network- and keychain-dependent.  The money-parsing logic is already thoroughly
//! unit-tested in `src/cli/money.rs`, so we intentionally skip binary-level amount
//! content tests here and only assert on the Clap required-argument path (which
//! fires before any credential lookup).

use assert_cmd::Command;
use predicates::prelude::*;

// ── Helper ───────────────────────────────────────────────────────────────────

fn flute() -> Command {
    Command::cargo_bin("flute").expect("binary must be compiled")
}

// ── transactions sale --help ──────────────────────────────────────────────────

/// `flute transactions sale --help` exits 0 and documents the key flags.
#[test]
fn transactions_sale_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["transactions", "sale", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--card").and(predicate::str::contains("--amount")));
}

// ── transactions auth --help ──────────────────────────────────────────────────

/// `flute transactions auth --help` exits 0 and documents the key flags.
#[test]
fn transactions_auth_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["transactions", "auth", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--card").and(predicate::str::contains("--amount")));
}

// ── transactions list --help ──────────────────────────────────────────────────

/// `flute transactions list --help` exits 0, documents --limit, --status,
/// and mentions the page-only filter caveat.
#[test]
fn transactions_list_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["transactions", "list", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--limit")
                .and(predicate::str::contains("--status"))
                // The help text notes that --status/--from/--to are page-only filters.
                .and(predicate::str::contains("page")),
        );
}

// ── transactions settle --help ────────────────────────────────────────────────

/// `flute transactions settle --help` exits 0, documents --payment-processor-id
/// and the batch-level semantics.
#[test]
fn transactions_settle_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["transactions", "settle", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--payment-processor-id")
                // The help text should mention "batch"
                .and(predicate::str::contains("batch")),
        );
}

// ── transactions inspect --help ───────────────────────────────────────────────

/// `flute transactions inspect --help` exits 0 and mentions the id argument.
#[test]
fn transactions_inspect_help_exits_zero() {
    flute()
        .args(["transactions", "inspect", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id"));
}

// ── transactions get --help ───────────────────────────────────────────────────

/// `flute transactions get --help` exits 0.
#[test]
fn transactions_get_help_exits_zero() {
    flute()
        .args(["transactions", "get", "--help"])
        .assert()
        .success();
}

// ── transactions capture --help ───────────────────────────────────────────────

/// `flute transactions capture --help` exits 0 and documents `--transaction-id`.
#[test]
fn transactions_capture_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["transactions", "capture", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--transaction-id"));
}

// ── transactions void --help ──────────────────────────────────────────────────

/// `flute transactions void --help` exits 0 and documents `--transaction-id`.
#[test]
fn transactions_void_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["transactions", "void", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--transaction-id"));
}

// ── transactions refund --help ────────────────────────────────────────────────

/// `flute transactions refund --help` exits 0 and documents `--transaction-id`
/// and `--card-data-source`.
#[test]
fn transactions_refund_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["transactions", "refund", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--transaction-id")
                .and(predicate::str::contains("--card-data-source")),
        );
}

// ── transactions tip-adjust --help ───────────────────────────────────────────

/// `flute transactions tip-adjust --help` exits 0 and documents
/// `--transaction-id` and `--tip-amount`.
#[test]
fn transactions_tip_adjust_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["transactions", "tip-adjust", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--transaction-id")
                .and(predicate::str::contains("--tip-amount")),
        );
}

// ── Required-argument enforcement (Clap, no network) ─────────────────────────

/// `flute transactions sale` with no `--amount` must exit non-zero (Clap
/// required-argument error) and mention "amount" or "required" in stderr.
///
/// This assertion fires entirely within the Clap argument parser — before any
/// credential or network path is reached — so it is always deterministic.
#[test]
fn transactions_sale_without_amount_fails_with_usage_error() {
    flute()
        .args(["transactions", "sale"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("amount").or(predicate::str::contains("required")));
}

/// `flute transactions auth` with no `--amount` must exit non-zero.
#[test]
fn transactions_auth_without_amount_fails_with_usage_error() {
    flute()
        .args(["transactions", "auth"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("amount").or(predicate::str::contains("required")));
}

/// `flute transactions get` with no positional `id` must exit non-zero (Clap
/// required positional argument missing).
#[test]
fn transactions_get_without_id_fails_with_usage_error() {
    flute().args(["transactions", "get"]).assert().failure();
}

/// `flute transactions capture` with no `--transaction-id` must exit non-zero.
#[test]
fn transactions_capture_without_transaction_id_fails() {
    flute()
        .args(["transactions", "capture"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("transaction-id").or(predicate::str::contains("required")),
        );
}

/// `flute transactions void` with no `--transaction-id` must exit non-zero.
#[test]
fn transactions_void_without_transaction_id_fails() {
    flute()
        .args(["transactions", "void"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("transaction-id").or(predicate::str::contains("required")),
        );
}

/// `flute transactions refund` with no `--transaction-id` must exit non-zero.
#[test]
fn transactions_refund_without_transaction_id_fails() {
    flute()
        .args(["transactions", "refund"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("transaction-id").or(predicate::str::contains("required")),
        );
}

/// `flute transactions settle` with no `--payment-processor-id` must exit non-zero.
#[test]
fn transactions_settle_without_processor_id_fails() {
    flute()
        .args(["transactions", "settle"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("payment-processor-id")
                .or(predicate::str::contains("required")),
        );
}

/// `flute transactions tip-adjust` with no required flags must exit non-zero.
#[test]
fn transactions_tip_adjust_without_required_flags_fails() {
    flute()
        .args(["transactions", "tip-adjust"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("transaction-id").or(predicate::str::contains("required")),
        );
}

/// `flute transactions inspect` with no positional `id` must exit non-zero.
#[test]
fn transactions_inspect_without_id_fails() {
    flute().args(["transactions", "inspect"]).assert().failure();
}
