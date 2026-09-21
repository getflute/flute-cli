//! Handlers for `flute auth {login,status,switch,logout,token}`.

use anyhow::Result;

use crate::auth;
use crate::cli::OutputFormat;
use crate::config::{self, Profile};

/// Prompt for client_id and client_secret, then store them in the OS keychain.
pub async fn login(profile: &str) -> Result<()> {
    use std::io::{self, BufRead, Write};
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    print!("client_id for [{profile}]: ");
    stdout.flush()?;
    let mut id = String::new();
    stdin.lock().read_line(&mut id)?;
    let id = id.trim().to_string();

    let secret = rpassword::prompt_password(format!("client_secret for [{profile}]: "))?;
    let secret = secret.trim().to_string();

    if id.is_empty() || secret.is_empty() {
        anyhow::bail!("client_id and client_secret are both required");
    }

    auth::keychain::store_client_credentials(profile, &id, &secret)?;
    println!("Stored credentials for profile [{profile}] in OS keychain.");
    Ok(())
}

/// Show active profile, environment, and credential/token status.
/// Report the active profile plus a **live** authentication check (ARISE-4706):
/// the current client ID and whether the stored credentials actually
/// authenticate against the API right now (with an authenticated ping).
pub async fn status(profile: &str, output: OutputFormat) -> Result<()> {
    let p =
        Profile::by_name(profile).ok_or_else(|| anyhow::anyhow!("unknown profile: {profile}"))?;

    let creds = auth::keychain::load_with_env_fallback(profile)?;
    // Client ID from stored creds; overridden by the authoritative value the
    // server echoes back on a successful ping.
    let mut client_id = creds.as_ref().map(|(id, _)| id.clone());
    let mut merchant_id: Option<String> = None;

    // Live check: `authenticated` is true only when we have credentials AND an
    // authenticated ping round-trips successfully. Any failure (no creds, bad
    // creds, network/timeout) leaves it false but still reports the client ID.
    let mut authenticated = false;
    if creds.is_some()
        && let Ok((_p, api)) = crate::build_client(profile)
        && let Ok(body) = api.ping().await
    {
        authenticated = body
            .get("authenticated")
            .and_then(|b| b.as_bool())
            .unwrap_or(true);
        if let Some(cid) = body.get("clientId").and_then(|v| v.as_str()) {
            client_id = Some(cid.to_string());
        }
        merchant_id = body
            .get("merchantId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
    }

    match output {
        OutputFormat::Json => {
            let data = serde_json::json!({
                "profile": p.name,
                "api_base_url": p.api_base_url,
                "authenticated": authenticated,
                "client_id": client_id,
                "merchant_id": merchant_id,
            });
            let env = crate::cli::output::Envelope::new("auth_status", data, &p.name, None);
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        OutputFormat::Quiet => {
            // id-only convention: the client ID (blank line when unknown).
            println!("{}", client_id.as_deref().unwrap_or(""));
        }
        OutputFormat::Table => {
            println!("Profile:       {}", p.name);
            println!("API base:      {}", p.api_base_url);
            println!("Authenticated: {authenticated}");
            println!("Client ID:     {}", client_id.as_deref().unwrap_or("—"));
            if let Some(m) = &merchant_id {
                println!("Merchant ID:   {m}");
            }
        }
    }

    Ok(())
}

/// Set the default profile in `~/.flute/config.toml`.
pub fn switch(new_profile: &str) -> Result<()> {
    validate_switch_target(new_profile)?;
    let mut cfg = config::load_or_default();
    cfg.default_profile = new_profile.to_string();
    config::save(&cfg)?;
    println!("Default profile set to [{new_profile}].");
    Ok(())
}

/// Clear stored credentials for the active profile.
pub fn logout(profile: &str) -> Result<()> {
    auth::keychain::delete_client_credentials(profile)?;
    println!("Credentials for profile [{profile}] removed from OS keychain.");
    Ok(())
}

/// Print the current bearer token (debugging aid).
pub async fn token(profile: &str) -> Result<()> {
    let (_p, api) = crate::build_client(profile)?;
    let bearer = api
        .tokens
        .bearer()
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("{bearer}");
    Ok(())
}

/// Validate that `profile` names a known profile.  Used by `switch` and tested
/// independently so the guard is exercised without touching the filesystem.
pub fn validate_switch_target(profile: &str) -> Result<()> {
    Profile::by_name(profile)
        .map(|_| ())
        .ok_or_else(|| anyhow::anyhow!("unknown profile: {profile}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn switch_validates_profile_name() {
        assert!(super::validate_switch_target("garbage").is_err());
        assert!(super::validate_switch_target("production").is_ok());
        assert!(super::validate_switch_target("prod").is_ok());
        assert!(super::validate_switch_target("sandbox").is_ok());
    }
}
