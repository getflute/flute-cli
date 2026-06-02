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
    Entry::new(SERVICE, profile)
        .with_context(|| format!("keyring entry for profile {profile}"))
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

/// Env vars win over the keychain — the recommended path for CI/agents.
pub fn load_with_env_fallback(profile: &str) -> Result<Option<(String, String)>> {
    if let (Ok(id), Ok(secret)) = (
        std::env::var("FLUTE_CLIENT_ID"),
        std::env::var("FLUTE_CLIENT_SECRET"),
    ) {
        return Ok(Some((id, secret)));
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
                assert_eq!(
                    got,
                    Some(("env-id".to_string(), "env-secret".to_string()))
                );
            },
        );
    }
}
