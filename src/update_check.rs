//! Lightweight startup check that pings GitHub Releases at most once per
//! 24 h and reports whether a newer version of flute is available.
//!
//! Three opt-out paths so this never gets in the way:
//!   1. `auto_update_check = false` in `~/.flute/config.toml`
//!   2. `FLUTE_NO_UPDATE_CHECK` env var set to anything
//!   3. `CI` env var set (typical Actions/Buildkite/Jenkins indicator)
//!
//! Callers are also expected to bail when stderr isn't a TTY — there is no
//! point printing an update notice to a piped or redirected stream — but
//! `IsTerminal` is a per-call concern so that check lives at the call site.
//!
//! The cache file lives at `~/.flute/update-check.json`. Anything we can't
//! parse, write, or fetch is treated as "no notice" rather than a hard
//! error: this code path is best-effort, not part of the user's task.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::config::{Config, config_dir};

const CACHE_FILE: &str = "update-check.json";
const CACHE_TTL_SECS: u64 = 24 * 60 * 60;
const NETWORK_TIMEOUT_SECS: u64 = 3;

#[derive(serde::Serialize, serde::Deserialize)]
struct Cache {
    checked_at_unix_secs: u64,
    /// Latest version seen at the time of the check; `None` means we
    /// confirmed the binary was on latest. We persist the answer either way
    /// so a "no update" check still suppresses the next 24 h of network
    /// calls.
    latest_version: Option<String>,
}

/// True if any of the documented opt-outs apply.
pub fn opt_out(cfg: &Config) -> bool {
    if !cfg.auto_update_check {
        return true;
    }
    if std::env::var_os("FLUTE_NO_UPDATE_CHECK").is_some() {
        return true;
    }
    if std::env::var_os("CI").is_some() {
        return true;
    }
    false
}

/// Returns `Some(latest_version)` if a newer version exists, otherwise `None`.
/// Uses the on-disk cache when fresh; falls back to a network query (bounded
/// by `NETWORK_TIMEOUT_SECS`) when the cache is stale or unreadable.
pub async fn check_for_update() -> Option<String> {
    if let Some(cached) = read_fresh_cache() {
        return cached.latest_version.filter(|v| is_newer_than_current(v));
    }

    let latest: Option<String> = tokio::time::timeout(
        Duration::from_secs(NETWORK_TIMEOUT_SECS),
        crate::update::query_latest_silently(),
    )
    .await
    .unwrap_or_default();

    let _ = write_cache(&Cache {
        checked_at_unix_secs: now_unix(),
        latest_version: latest.clone(),
    });

    latest.filter(|v| is_newer_than_current(v))
}

/// Returns true only if `v` is *strictly newer* than the compiled binary
/// version, compared as semver. A bare string `!=` check would (incorrectly)
/// say "update available" when a stale cache held an older version than the
/// running binary — e.g. cache "0.5.3" after the user upgraded to 0.5.4.
/// If either side fails to parse as semver, fall back to false so a parse
/// glitch can't trigger a spurious update prompt.
fn is_newer_than_current(v: &str) -> bool {
    let Ok(current) = env!("CARGO_PKG_VERSION").parse::<axoupdater::Version>() else {
        return false;
    };
    let Ok(candidate) = v.parse::<axoupdater::Version>() else {
        return false;
    };
    candidate > current
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn cache_path() -> std::path::PathBuf {
    config_dir().join(CACHE_FILE)
}

fn read_fresh_cache() -> Option<Cache> {
    read_fresh_cache_at(&cache_path(), now_unix())
}

fn write_cache(c: &Cache) -> std::io::Result<()> {
    write_cache_at(&cache_path(), c)
}

/// Path-injectable cache reader so tests don't have to touch `~/.flute`.
/// Returns `None` if the file is missing, corrupt, or older than the TTL.
fn read_fresh_cache_at(path: &std::path::Path, now_secs: u64) -> Option<Cache> {
    let raw = std::fs::read_to_string(path).ok()?;
    let cache: Cache = serde_json::from_str(&raw).ok()?;
    let age = now_secs.saturating_sub(cache.checked_at_unix_secs);
    (age < CACHE_TTL_SECS).then_some(cache)
}

/// Path-injectable cache writer. Creates the parent directory if needed.
fn write_cache_at(path: &std::path::Path, c: &Cache) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string(c)?)
}

/// Compose the one-line notice we print when a newer version is available.
pub fn notice_for(version: &str) -> String {
    format!(
        "A newer version ({version}) of flute is available \u{2014} run `flute update` to install."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opt_out_respects_config_flag() {
        let cfg = Config {
            auto_update_check: false,
            ..Config::default()
        };
        assert!(opt_out(&cfg));
    }

    #[test]
    fn opt_out_respects_env_var() {
        temp_env::with_var("FLUTE_NO_UPDATE_CHECK", Some("1"), || {
            let cfg = Config::default();
            assert!(opt_out(&cfg));
        });
    }

    #[test]
    fn opt_out_respects_ci_env_var() {
        temp_env::with_var("CI", Some("true"), || {
            let cfg = Config::default();
            assert!(opt_out(&cfg));
        });
    }

    #[test]
    fn opt_out_false_when_all_clear() {
        temp_env::with_vars(
            [
                ("FLUTE_NO_UPDATE_CHECK", None::<&str>),
                ("CI", None::<&str>),
            ],
            || {
                let cfg = Config {
                    auto_update_check: true,
                    ..Config::default()
                };
                assert!(!opt_out(&cfg));
            },
        );
    }

    #[test]
    fn current_version_is_not_considered_newer() {
        assert!(!is_newer_than_current(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn strictly_newer_semver_counts_as_newer() {
        assert!(is_newer_than_current("999.999.999"));
    }

    #[test]
    fn strictly_older_semver_is_not_newer() {
        // Regression for the "stale cache shows update available" bug:
        // when the user upgraded to 0.5.4 but the cache still held
        // latest_version="0.5.3" (written before the v0.5.4 tag landed
        // on GitHub Releases), the old string-`!=` check incorrectly
        // surfaced a notice. Semver `>` correctly returns false.
        assert!(!is_newer_than_current("0.0.0"));
    }

    #[test]
    fn unparseable_candidate_returns_false() {
        // If the GitHub API ever returns something we can't parse as
        // semver, prefer no notice over a spurious one.
        assert!(!is_newer_than_current("not-a-version"));
        assert!(!is_newer_than_current(""));
    }

    #[test]
    fn notice_contains_version_and_update_command() {
        let n = notice_for("9.9.9");
        assert!(n.contains("9.9.9"), "notice should embed the version: {n}");
        assert!(
            n.contains("flute update"),
            "notice should tell the user how to update: {n}"
        );
    }

    #[test]
    fn cache_round_trip_returns_value_when_fresh() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-check.json");
        let now = 1_700_000_000;
        let written = Cache {
            checked_at_unix_secs: now,
            latest_version: Some("9.9.9".into()),
        };
        write_cache_at(&path, &written).unwrap();
        let read = read_fresh_cache_at(&path, now + 60).expect("cache should still be fresh");
        assert_eq!(read.latest_version.as_deref(), Some("9.9.9"));
    }

    #[test]
    fn cache_older_than_ttl_is_treated_as_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-check.json");
        let then = 1_700_000_000;
        write_cache_at(
            &path,
            &Cache {
                checked_at_unix_secs: then,
                latest_version: Some("9.9.9".into()),
            },
        )
        .unwrap();
        let now = then + CACHE_TTL_SECS + 1;
        assert!(
            read_fresh_cache_at(&path, now).is_none(),
            "cache older than TTL must force a re-check"
        );
    }

    #[test]
    fn cache_with_no_update_still_persists_to_suppress_followups() {
        // A "you're on latest" check writes a Cache with latest_version=None.
        // The read path must round-trip that so we don't hit the network
        // again for 24h after confirming no update is available.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-check.json");
        let now = 1_700_000_000;
        write_cache_at(
            &path,
            &Cache {
                checked_at_unix_secs: now,
                latest_version: None,
            },
        )
        .unwrap();
        let read = read_fresh_cache_at(&path, now + 60).expect("fresh");
        assert!(read.latest_version.is_none());
    }

    #[test]
    fn corrupt_cache_is_ignored_rather_than_panicking() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-check.json");
        std::fs::write(&path, "not-json").unwrap();
        assert!(read_fresh_cache_at(&path, 1_700_000_000).is_none());
    }
}
