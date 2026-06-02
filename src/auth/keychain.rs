//! Client credentials in the OS keychain — one JSON entry per profile.

use anyhow::{Context, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};

const SERVICE: &str = "flute-cli";

#[derive(Debug, Serialize, Deserialize)]
struct StoredCreds {
    client_id: String,
    client_secret: String,
}

fn entry(profile: &str) -> Result<Entry> {
    Entry::new(SERVICE, profile).with_context(|| format!("keyring entry for profile {profile}"))
}

pub fn store_client_credentials(profile: &str, client_id: &str, client_secret: &str) -> Result<()> {
    let json = serde_json::to_string(&StoredCreds {
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
    })
    .context("serialising credentials")?;
    entry(profile)?.set_password(&json)?;
    Ok(())
}

pub fn load_client_credentials(profile: &str) -> Result<Option<(String, String)>> {
    match entry(profile)?.get_password() {
        Ok(json) => {
            let creds: StoredCreds =
                serde_json::from_str(&json).context("decoding credentials JSON")?;
            Ok(Some((creds.client_id, creds.client_secret)))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn delete_client_credentials(profile: &str) -> Result<()> {
    let _ = entry(profile)?.delete_credential();
    Ok(())
}

/// Returns `(client_id, client_secret)` from environment variables if both are set.
///
/// Returns `None` if either variable is absent or empty. This is a pure helper
/// with no I/O so it can be tested without touching the keychain.
pub(crate) fn creds_from_env() -> Option<(String, String)> {
    match (
        std::env::var("FLUTE_CLIENT_ID"),
        std::env::var("FLUTE_CLIENT_SECRET"),
    ) {
        (Ok(id), Ok(secret)) => Some((id, secret)),
        _ => None,
    }
}

/// Env vars win over the keychain — the recommended path for CI/agents.
pub fn load_with_env_fallback(profile: &str) -> Result<Option<(String, String)>> {
    if let Some(creds) = creds_from_env() {
        return Ok(Some(creds));
    }
    load_client_credentials(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_fallback_returns_env_creds() {
        temp_env::with_vars(
            [
                ("FLUTE_CLIENT_ID", Some("env-id")),
                ("FLUTE_CLIENT_SECRET", Some("env-secret")),
            ],
            || {
                let got = load_with_env_fallback("sandbox").unwrap();
                assert_eq!(got, Some(("env-id".to_string(), "env-secret".to_string())));
            },
        );
    }

    #[test]
    fn creds_from_env_both_set_returns_pair() {
        temp_env::with_vars(
            [
                ("FLUTE_CLIENT_ID", Some("ci-id")),
                ("FLUTE_CLIENT_SECRET", Some("ci-secret")),
            ],
            || {
                assert_eq!(
                    creds_from_env(),
                    Some(("ci-id".to_string(), "ci-secret".to_string()))
                );
            },
        );
    }

    #[test]
    fn creds_from_env_only_id_set_returns_none() {
        temp_env::with_vars(
            [
                ("FLUTE_CLIENT_ID", Some("ci-id")),
                ("FLUTE_CLIENT_SECRET", None),
            ],
            || {
                assert_eq!(creds_from_env(), None);
            },
        );
    }

    #[test]
    fn creds_from_env_neither_set_returns_none() {
        temp_env::with_vars(
            [
                ("FLUTE_CLIENT_ID", None::<&str>),
                ("FLUTE_CLIENT_SECRET", None::<&str>),
            ],
            || {
                assert_eq!(creds_from_env(), None);
            },
        );
    }
}
