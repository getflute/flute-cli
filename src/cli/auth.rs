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
pub async fn status(profile: &str, output: OutputFormat) -> Result<()> {
    let p =
        Profile::by_name(profile).ok_or_else(|| anyhow::anyhow!("unknown profile: {profile}"))?;

    let creds = auth::keychain::load_with_env_fallback(profile)?;
    let has_creds = creds.is_some();

    match output {
        OutputFormat::Json => {
            let data = serde_json::json!({
                "profile": p.name,
                "api_base_url": p.api_base_url,
                "has_credentials": has_creds,
            });
            let env = crate::cli::output::Envelope::new("auth_status", data, &p.name, None);
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        OutputFormat::Quiet => {
            println!("{}", p.name);
        }
        OutputFormat::Table => {
            println!("Profile:     {}", p.name);
            println!("API base:    {}", p.api_base_url);
            println!(
                "Credentials: {}",
                if has_creds {
                    "found"
                } else {
                    "not set — run `flute auth login`"
                }
            );
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
