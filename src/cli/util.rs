//! Handlers for `flute ping` and `flute version`.

use anyhow::Result;

use crate::build_client;
use crate::cli::output::{Envelope, OutputFormat};

/// Call the API health endpoint and print the result.
pub async fn ping(profile: &str, output: OutputFormat) -> Result<()> {
    let (p, api) = build_client(profile)?;
    let body = api.ping().await?;

    match output {
        OutputFormat::Json => {
            let env = Envelope::new("ping", body, &p.name, None);
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        OutputFormat::Quiet => {
            // Quiet mode: just indicate success.
            println!("ok");
        }
        OutputFormat::Table => {
            println!("ping  ok  (profile={})", p.name);
        }
    }

    Ok(())
}

/// Print the CLI version and active profile.
pub fn version(profile: &str, output: OutputFormat) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let p = crate::config::Profile::by_name(profile)
        .ok_or_else(|| anyhow::anyhow!("unknown profile: {profile}"))?;

    match output {
        OutputFormat::Json => {
            let data = serde_json::json!({
                "version": version,
                "profile": p.name,
                "api_base_url": p.api_base_url,
            });
            let env = Envelope::new("version", data, &p.name, None);
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        OutputFormat::Quiet => {
            println!("{version}");
        }
        OutputFormat::Table => {
            println!("flute  v{version}");
            println!("Profile:  {}", p.name);
            println!("API base: {}", p.api_base_url);
        }
    }

    Ok(())
}
