#![forbid(unsafe_code)]

pub mod api;
pub mod auth;
pub mod cli;
pub mod config;

use clap::{CommandFactory, Parser};

/// Pure helper: build the production warning banner string.
///
/// Returns `Some(banner)` when `profile` is production, `None` for sandbox.
/// When `color` is true the banner is wrapped in ANSI red escape codes.
/// I/O (eprintln + is_terminal) is intentionally kept out of this function
/// so it stays unit-testable without touching stderr.
pub(crate) fn production_banner(profile: &config::Profile, color: bool) -> Option<String> {
    if !profile.is_production() {
        return None;
    }
    let msg = format!("⚠ Operating on PRODUCTION ({})", profile.api_base_url);
    if color {
        Some(format!("\x1b[31m{msg}\x1b[0m"))
    } else {
        Some(msg)
    }
}

/// Pure helper: build an `ApiError::Auth` for missing credentials.
///
/// Extracted so unit tests can assert on exit code and JSON kind without
/// hitting the OS keychain or network.
pub(crate) fn missing_credentials_error(profile: &str) -> anyhow::Error {
    api::ApiError::Auth(format!(
        "no credentials for [{profile}]; run `flute auth login`"
    ))
    .into()
}

/// Build an authenticated `ApiClient` from stored/env credentials for `profile`.
///
/// Returns `(Profile, ApiClient)` so callers can embed `profile.name` in
/// output envelopes and error messages without an extra lookup.
pub(crate) fn build_client(profile: &str) -> anyhow::Result<(config::Profile, api::ApiClient)> {
    use std::io::IsTerminal;
    use std::sync::Arc;
    use std::time::Duration;

    let p = config::Profile::by_name(profile)
        .ok_or_else(|| anyhow::anyhow!("unknown profile: {profile}"))?;

    // Emit the production warning before attempting any network call so that
    // every authenticated command (ping, auth token, future resource commands)
    // gets the banner from a single place.
    if let Some(banner) = production_banner(&p, std::io::stderr().is_terminal()) {
        eprintln!("{banner}");
    }

    let (id, secret) = auth::keychain::load_with_env_fallback(profile)?
        .ok_or_else(|| missing_credentials_error(profile))?;
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let fetcher = Arc::new(auth::token::OAuth2Fetcher::new(
        p.oauth_url.clone(),
        id,
        secret,
        http.clone(),
    ));
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
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if debug {
            "debug,flute_cli=debug,reqwest=debug,hyper=info".into()
        } else {
            "warn,flute_cli=info".into()
        }
    });

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

/// Resolve the effective output format from the precedence chain:
/// flag (`--output`) → config-file `output` → default (`Table`).
///
/// Extracted as a pure function so it can be unit-tested without parsing
/// command-line arguments or touching the filesystem.
pub(crate) fn resolve_output(
    flag: Option<cli::OutputFormat>,
    config_value: &str,
) -> cli::OutputFormat {
    flag.or_else(|| cli::OutputFormat::from_config_str(config_value))
        .unwrap_or(cli::OutputFormat::Table)
}

async fn dispatch_transactions(
    profile: &str,
    output_fmt: cli::OutputFormat,
    tc: cli::TransactionsCommand,
) -> anyhow::Result<()> {
    use cli::TransactionsCommand;
    use cli::money::parse_amount;

    match tc {
        TransactionsCommand::Sale {
            amount,
            card,
            exp,
            cvv,
            tip_amount,
            customer_id,
            payment_method_id,
            currency_id,
            card_data_source,
            l2_tax_rate,
            l3_invoice,
            l3_po,
            l3_product,
            reference_id,
        } => {
            let amount = parse_amount(&amount)?;
            let tip_amount = tip_amount.as_deref().map(parse_amount).transpose()?;
            let l2_tax_rate = l2_tax_rate.as_deref().map(parse_amount).transpose()?;
            cli::transactions::sale(
                profile,
                output_fmt,
                amount,
                card,
                exp,
                cvv,
                tip_amount,
                customer_id,
                payment_method_id,
                currency_id,
                card_data_source,
                l2_tax_rate,
                l3_invoice,
                l3_po,
                l3_product,
                reference_id,
            )
            .await
        }
        TransactionsCommand::Auth {
            amount,
            card,
            exp,
            cvv,
            tip_amount,
            customer_id,
            payment_method_id,
            currency_id,
            card_data_source,
            l2_tax_rate,
            l3_invoice,
            l3_po,
            l3_product,
            reference_id,
        } => {
            let amount = parse_amount(&amount)?;
            let tip_amount = tip_amount.as_deref().map(parse_amount).transpose()?;
            let l2_tax_rate = l2_tax_rate.as_deref().map(parse_amount).transpose()?;
            cli::transactions::auth_txn(
                profile,
                output_fmt,
                amount,
                card,
                exp,
                cvv,
                tip_amount,
                customer_id,
                payment_method_id,
                currency_id,
                card_data_source,
                l2_tax_rate,
                l3_invoice,
                l3_po,
                l3_product,
                reference_id,
            )
            .await
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    let profile = cli.profile.clone();
    let debug = cli.debug;

    // Precedence: --output flag → config-file output → Table.
    let cfg = config::load_or_default();
    let output_fmt = resolve_output(cli.output, &cfg.output);

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
            cli::Command::Transactions(tc) => {
                dispatch_transactions(&profile, output_fmt, *tc).await
            }
        };

        // On failure: always call process::exit with the semantic exit code.
        // Under --output json: additionally print the structured error envelope
        // to stdout so the agent's stdout stream stays pure JSON.
        // Under table/quiet: print a human-readable message to stderr instead.
        if let Err(ref e) = dispatch_result {
            if output_fmt == cli::OutputFormat::Json {
                let envelope = cli::output::ErrorJson::from_anyhow(e);
                if let Ok(json) = serde_json::to_string_pretty(&envelope) {
                    println!("{json}");
                }
            } else {
                eprintln!("Error: {e:#}");
            }
            std::process::exit(cli::output::exit_code_for(e));
        }

        dispatch_result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::OutputFormat;
    use crate::config::Profile;

    // ── Change 1: production_banner ──────────────────────────────────────────

    #[test]
    fn production_banner_returns_some_for_production() {
        let p = Profile::production();
        let banner = production_banner(&p, false);
        assert!(banner.is_some());
        let s = banner.unwrap();
        assert!(s.contains("PRODUCTION"));
        assert!(s.contains(&p.api_base_url));
    }

    #[test]
    fn production_banner_returns_none_for_sandbox() {
        let p = Profile::sandbox();
        assert!(production_banner(&p, false).is_none());
        assert!(production_banner(&p, true).is_none());
    }

    #[test]
    fn production_banner_with_color_wraps_ansi_red() {
        let p = Profile::production();
        let colored = production_banner(&p, true).unwrap();
        assert!(colored.starts_with("\x1b[31m"));
        assert!(colored.ends_with("\x1b[0m"));
        assert!(colored.contains("PRODUCTION"));
    }

    #[test]
    fn production_banner_plain_has_no_ansi() {
        let p = Profile::production();
        let plain = production_banner(&p, false).unwrap();
        assert!(!plain.contains('\x1b'));
    }

    // ── Change 2: resolve_output (config-file precedence) ───────────────────

    #[test]
    fn output_format_from_config_str_parses_all_variants() {
        assert_eq!(
            OutputFormat::from_config_str("table"),
            Some(OutputFormat::Table)
        );
        assert_eq!(
            OutputFormat::from_config_str("json"),
            Some(OutputFormat::Json)
        );
        assert_eq!(
            OutputFormat::from_config_str("quiet"),
            Some(OutputFormat::Quiet)
        );
    }

    #[test]
    fn output_format_from_config_str_is_case_insensitive() {
        assert_eq!(
            OutputFormat::from_config_str("JSON"),
            Some(OutputFormat::Json)
        );
        assert_eq!(
            OutputFormat::from_config_str("Table"),
            Some(OutputFormat::Table)
        );
        assert_eq!(
            OutputFormat::from_config_str("QUIET"),
            Some(OutputFormat::Quiet)
        );
    }

    #[test]
    fn output_format_from_config_str_rejects_garbage() {
        assert_eq!(OutputFormat::from_config_str("yaml"), None);
        assert_eq!(OutputFormat::from_config_str(""), None);
        assert_eq!(OutputFormat::from_config_str("csv"), None);
    }

    #[test]
    fn resolve_output_flag_beats_config() {
        // Flag present: always wins regardless of config-file value.
        assert_eq!(
            resolve_output(Some(OutputFormat::Json), "table"),
            OutputFormat::Json
        );
        assert_eq!(
            resolve_output(Some(OutputFormat::Quiet), "json"),
            OutputFormat::Quiet
        );
    }

    #[test]
    fn resolve_output_falls_back_to_config_when_no_flag() {
        assert_eq!(resolve_output(None, "json"), OutputFormat::Json);
        assert_eq!(resolve_output(None, "quiet"), OutputFormat::Quiet);
    }

    #[test]
    fn resolve_output_defaults_to_table_when_nothing_set() {
        // No flag, garbage config value → default Table.
        assert_eq!(resolve_output(None, ""), OutputFormat::Table);
        assert_eq!(resolve_output(None, "unknown"), OutputFormat::Table);
    }

    // ── Change 3: missing_credentials_error classification ──────────────────

    #[test]
    fn missing_credentials_exit_code_is_2() {
        let e = missing_credentials_error("sandbox");
        assert_eq!(crate::cli::output::exit_code_for(&e), 2);
    }

    #[test]
    fn missing_credentials_error_json_kind_is_auth() {
        let e = missing_credentials_error("sandbox");
        let ej = crate::cli::output::ErrorJson::from_anyhow(&e);
        assert_eq!(ej.kind, "auth");
        assert!(ej.message.contains("sandbox"));
        assert!(ej.message.contains("flute auth login"));
    }
}
