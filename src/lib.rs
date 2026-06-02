#![forbid(unsafe_code)]

pub mod api;
pub mod auth;
pub mod cli;
pub mod config;

use clap::{CommandFactory, Parser};

/// Build an authenticated `ApiClient` from stored/env credentials for `profile`.
///
/// Returns `(Profile, ApiClient)` so callers can embed `profile.name` in
/// output envelopes and error messages without an extra lookup.
pub(crate) fn build_client(profile: &str) -> anyhow::Result<(config::Profile, api::ApiClient)> {
    use std::sync::Arc;
    use std::time::Duration;

    let p = config::Profile::by_name(profile)
        .ok_or_else(|| anyhow::anyhow!("unknown profile: {profile}"))?;
    let (id, secret) = auth::keychain::load_with_env_fallback(profile)?
        .ok_or_else(|| anyhow::anyhow!("no credentials for [{profile}]; run `flute auth login`"))?;
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let fetcher = Arc::new(auth::token::OAuth2Fetcher {
        oauth_url: p.oauth_url.clone(),
        client_id: id,
        client_secret: secret,
        http: http.clone(),
    });
    let api = api::ApiClient {
        base_url: p.api_base_url.clone(),
        http,
        tokens: auth::token::TokenStore::new(fetcher),
    };
    Ok((p, api))
}

/// Route tracing output based on the `debug` flag:
///   debug=true  → stdout at DEBUG level (per spec: "outputs every HTTP call to stdout")
///   debug=false → stderr at WARN/INFO
///
/// `RUST_LOG` always overrides the default filter when set.
fn init_tracing(debug: bool) {
    let env_filter = if debug {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "debug,flute_cli=debug,reqwest=debug,hyper=info".into())
    } else {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "warn,flute_cli=info".into())
    };

    if debug {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_writer(std::io::stdout)
            .with_ansi(false)
            .try_init();
    } else {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_writer(std::io::stderr)
            .try_init();
    }
}

pub fn run() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    let profile = cli.profile.clone();
    let debug = cli.debug;
    let output_fmt = cli.output;

    // No subcommand → print help and exit cleanly.
    let Some(cmd) = cli.command else {
        cli::Cli::command().print_help()?;
        println!();
        return Ok(());
    };

    init_tracing(debug);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let dispatch_result = match cmd {
            cli::Command::Auth(cli::AuthCommand::Login) => cli::auth::login(&profile).await,
            cli::Command::Auth(cli::AuthCommand::Status) => {
                cli::auth::status(&profile, output_fmt).await
            }
            cli::Command::Auth(cli::AuthCommand::Switch {
                profile: new_profile,
            }) => cli::auth::switch(&new_profile),
            cli::Command::Auth(cli::AuthCommand::Logout) => cli::auth::logout(&profile),
            cli::Command::Auth(cli::AuthCommand::Token) => cli::auth::token(&profile).await,
            cli::Command::Ping => cli::util::ping(&profile, output_fmt).await,
            cli::Command::Version => cli::util::version(&profile, output_fmt),
        };

        // Structured error envelope for agents: when --output json is set and
        // the command failed, print a JSON object to stdout and exit non-zero.
        // This keeps the agent's stdout stream pure JSON on both success and
        // failure paths.
        if let Err(ref e) = dispatch_result
            && output_fmt == cli::OutputFormat::Json
        {
            let envelope = cli::output::ErrorJson::from_anyhow(e);
            if let Ok(json) = serde_json::to_string_pretty(&envelope) {
                println!("{json}");
            }
            std::process::exit(cli::output::exit_code_for(e));
        }

        dispatch_result
    })
}
