//! Binary-level integration tests for the `settlements` subcommand group.
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
// in tests/customers.rs and tests/pos.rs.

use assert_cmd::Command;
use predicates::prelude::*;

// ── Helper ───────────────────────────────────────────────────────────────────

fn flute() -> Command {
    Command::cargo_bin("flute").expect("binary must be compiled")
}

// ── settlements list --help ───────────────────────────────────────────────────

/// `flute settlements list --help` exits 0 and documents --limit, --from,
/// --to, and --status.
#[test]
fn settlements_list_help_exits_zero_and_mentions_key_flags() {
    flute()
        .args(["settlements", "list", "--help"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("--limit")
                .and(predicate::str::contains("--from"))
                .and(predicate::str::contains("--to"))
                .and(predicate::str::contains("--status")),
        );
}

// ── settlements get --help ────────────────────────────────────────────────────

/// `flute settlements get --help` exits 0 and mentions the positional id
/// argument.
#[test]
fn settlements_get_help_exits_zero_and_mentions_id() {
    flute()
        .args(["settlements", "get", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id"));
}

// ── Required-argument enforcement (Clap, no network) ─────────────────────────

/// `flute settlements get` with no positional `id` must exit non-zero (Clap
/// required positional argument missing).
#[test]
fn settlements_get_without_id_fails() {
    flute().args(["settlements", "get"]).assert().failure();
}

// ── Status validation (client-side, no network) ───────────────────────────────

/// `flute settlements list --status bogus` must exit non-zero and mention the
/// valid values (`open`, `settled`) in stderr.  This exercises the client-side
/// validation guard in lib.rs before any network call is made.
#[test]
fn settlements_list_bogus_status_fails_with_valid_values() {
    flute()
        .args(["settlements", "list", "--status", "bogus"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("open").and(predicate::str::contains("settled")));
}
