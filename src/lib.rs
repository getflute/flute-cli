#![forbid(unsafe_code)]

pub mod api;
pub mod auth;
pub mod cli;
pub mod config;
pub mod update;
pub mod update_check;

use clap::{CommandFactory, Parser};
use std::io::IsTerminal;

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

/// Treat a 404 API error on a delete/remove operation as idempotent success.
///
/// Per spec: "treat 404 on re-delete as idempotent success" — a second delete of
/// an already-deleted resource should exit 0, not fail.  All other errors pass
/// through unchanged.
///
/// Extracted as a pure helper so it is independently unit-testable without
/// needing a live `ApiClient`.
pub(crate) fn treat_404_as_ok(result: Result<(), api::ApiError>) -> anyhow::Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(api::ApiError::Api { status: 404, .. }) => Ok(()),
        Err(e) => Err(e.into()),
    }
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
///   debug=true  → stderr at DEBUG level (HTTP traces go to stderr so `--output json` stdout stays parseable)
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
            .with_writer(std::io::stderr)
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

/// Parsed money fields common to every card-transaction verb.
struct ParsedMoneyArgs {
    amount: rust_decimal::Decimal,
    tip_amount: Option<rust_decimal::Decimal>,
    l2_tax_rate: Option<rust_decimal::Decimal>,
}

/// Parse the three money strings that every card-transaction verb carries.
///
/// Extracted to avoid duplicating three `parse_amount` / `.transpose()` calls
/// in every match arm.  As new verbs are added (capture, refund, …) this helper
/// keeps the parse logic in one place.
fn parse_txn_money(
    amount: &str,
    tip_amount: Option<&str>,
    l2_tax_rate: Option<&str>,
) -> anyhow::Result<ParsedMoneyArgs> {
    use cli::money::parse_amount;
    Ok(ParsedMoneyArgs {
        amount: parse_amount(amount)?,
        tip_amount: tip_amount.map(parse_amount).transpose()?,
        l2_tax_rate: l2_tax_rate.map(parse_amount).transpose()?,
    })
}

async fn dispatch_subscriptions(
    profile: &str,
    output_fmt: cli::OutputFormat,
    sc: cli::SubscriptionsCommand,
) -> anyhow::Result<()> {
    use cli::SubscriptionsCommand;
    use cli::subscriptions::{
        CreateArgs, build_subscription_body, filter_subscriptions_by_status, render_subscription,
        render_subscription_list, render_subscription_payments,
    };

    match sc {
        SubscriptionsCommand::Create {
            customer_id,
            payment_method_id,
            amount,
            currency_id,
            number_of_payments,
            payment_frequency,
            interval,
            transaction_type,
            requester_ip,
            payment_processor_id,
            start_date,
            sec_code,
            faster,
        } => {
            let args = CreateArgs {
                customer_id,
                payment_method_id,
                amount,
                currency_id,
                number_of_payments,
                payment_frequency,
                interval,
                transaction_type,
                requester_ip,
                payment_processor_id,
                start_date,
                sec_code,
                faster,
            };
            let body = build_subscription_body(&args)?;
            let (p, api) = build_client(profile)?;
            let result = api.create_subscription(body).await?;
            render_subscription(&result, output_fmt, &p.name)
        }
        SubscriptionsCommand::Get { id } => {
            let (p, api) = build_client(profile)?;
            let result = api.get_subscription(&id).await?;
            render_subscription(&result, output_fmt, &p.name)
        }
        SubscriptionsCommand::List {
            limit,
            page,
            search,
            customer_id,
            status,
        } => {
            let (p, api) = build_client(profile)?;
            let mut result = api
                .list_subscriptions(page, Some(limit), search.as_deref(), customer_id.as_deref())
                .await?;
            if let Some(ref s) = status {
                let items = result
                    .get("items")
                    .and_then(|x| x.as_array())
                    .cloned()
                    .unwrap_or_default();
                let filtered = filter_subscriptions_by_status(items, s);
                if let Some(obj) = result.as_object_mut() {
                    obj.insert("items".into(), serde_json::Value::Array(filtered));
                }
            }
            render_subscription_list(&result, output_fmt, &p.name)
        }
        SubscriptionsCommand::Payments { id } => {
            let (p, api) = build_client(profile)?;
            let result = api.subscription_payments(&id).await?;
            render_subscription_payments(&result, output_fmt, &p.name)
        }
        SubscriptionsCommand::Terminate { id, yes } => {
            if !yes {
                anyhow::bail!(
                    "termination requires --yes to confirm (e.g. `subscriptions terminate {id} --yes`)"
                );
            }
            let (p, api) = build_client(profile)?;
            let result = api.terminate_subscription(&id).await?;
            render_subscription(&result, output_fmt, &p.name)
        }
    }
}

async fn dispatch_settlements(
    profile: &str,
    output_fmt: cli::OutputFormat,
    sc: cli::SettlementsCommand,
) -> anyhow::Result<()> {
    use cli::SettlementsCommand;
    use cli::settlements::{render_settlement, render_settlement_list};

    match sc {
        SettlementsCommand::List {
            limit,
            page,
            from,
            to,
            status,
        } => {
            // Map --status flag to statusId integer.
            let status_id = match status.as_deref() {
                Some("open") => Some(1u32),
                Some("settled") => Some(2u32),
                Some(other) => {
                    anyhow::bail!("unknown status '{other}'; valid values: open, settled");
                }
                None => None,
            };
            let (p, api) = build_client(profile)?;
            let result = api
                .list_settlement_batches(
                    page,
                    Some(limit),
                    from.as_deref(),
                    to.as_deref(),
                    status_id,
                )
                .await?;
            render_settlement_list(&result, output_fmt, &p.name)
        }
        SettlementsCommand::Get { id } => {
            // No single-batch endpoint: fetch a large page and filter client-side.
            let (p, api) = build_client(profile)?;
            let result = api
                .list_settlement_batches(None, Some(100), None, None, None)
                .await?;
            let items = result
                .get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let found = items.into_iter().find(|item| {
                item.get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s == id)
                    .unwrap_or(false)
            });
            match found {
                Some(batch) => render_settlement(&batch, output_fmt, &p.name),
                None => anyhow::bail!(
                    "settlement batch {id} not found in current page (no single-batch endpoint)"
                ),
            }
        }
    }
}

async fn dispatch_tokens(
    profile: &str,
    output_fmt: cli::OutputFormat,
    tc: cli::TokensCommand,
) -> anyhow::Result<()> {
    use cli::TokensCommand;
    use cli::tokens::{build_token_body, render_token_create, render_token_list};

    match tc {
        TokensCommand::Create { merchant_id, name } => {
            let body = build_token_body(&merchant_id, &name);
            let (p, api) = build_client(profile)?;
            let result = api.create_token(body).await?;
            render_token_create(&result, output_fmt, &p.name)
        }
        TokensCommand::List { merchant_id } => {
            let (p, api) = build_client(profile)?;
            let result = api.list_tokens(merchant_id.as_deref()).await?;
            render_token_list(&result, output_fmt, &p.name)
        }
        TokensCommand::Revoke {
            client_id,
            merchant_id,
            yes,
        } => {
            if !yes {
                anyhow::bail!(
                    "revocation requires --yes to confirm (e.g. `tokens revoke --client-id {client_id} --merchant-id {merchant_id} --yes`)"
                );
            }
            let (_p, api) = build_client(profile)?;
            treat_404_as_ok(api.revoke_token(&client_id, &merchant_id).await)?;
            match output_fmt {
                cli::OutputFormat::Json => {} // empty stdout, exit 0
                cli::OutputFormat::Table => println!("Revoked token {client_id}."),
                cli::OutputFormat::Quiet => println!("{client_id}"),
            }
            Ok(())
        }
    }
}

async fn dispatch_pos(
    profile: &str,
    output_fmt: cli::OutputFormat,
    pc: cli::PosCommand,
) -> anyhow::Result<()> {
    use cli::PosCommand;
    use cli::pos::{
        PollOutcome, PosCreateArgs, build_pos_create_body, pos_id, pos_status,
        render_pos_transaction, render_pos_transaction_list, run_wait_poll,
    };

    match pc {
        PosCommand::Create {
            terminal_id,
            amount,
            transaction_type,
            currency_id,
            tip_amount,
            tip_rate,
            pos_device_id,
            reference_id,
            payment_processor_id,
            customer_id,
            target_transaction_id,
            reading_method,
            wait,
            wait_timeout,
        } => {
            let args = PosCreateArgs {
                terminal_id,
                transaction_type,
                amount,
                currency_id,
                tip_amount,
                tip_rate,
                pos_device_id,
                reference_id,
                payment_processor_id,
                customer_id,
                target_transaction_id,
                reading_method,
                wait,
            };
            let body = build_pos_create_body(&args)?;
            let (p, api) = build_client(profile)?;
            let created = api.create_pos_transaction(body).await?;

            if !wait {
                return render_pos_transaction(&created, output_fmt, &p.name);
            }

            // Extract the POS transaction id to use for polling.
            // create/cancel responses use `posTransactionId`; get/list use `id`.
            let pos_txn_id = pos_id(&created)
                .ok_or_else(|| anyhow::anyhow!("create response missing id (id/posTransactionId)"))?
                .to_string();

            // Clone the api for the closure (ApiClient is Clone).
            let api_clone = api.clone();
            let getter = move |id: String| {
                let api = api_clone.clone();
                async move {
                    api.get_pos_transaction(&id)
                        .await
                        .map_err(anyhow::Error::from)
                }
            };

            let outcome = run_wait_poll(&pos_txn_id, wait_timeout, getter).await?;

            match outcome {
                PollOutcome::Completed(v) => {
                    render_pos_transaction(&v, output_fmt, &p.name)?;
                }
                PollOutcome::TimedOut(v) => {
                    // Print what we know, then indicate timeout on stderr.
                    render_pos_transaction(&v, output_fmt, &p.name)?;
                    eprintln!(
                        "Warning: --wait-timeout ({wait_timeout}s) expired; last status shown above."
                    );
                    std::process::exit(1);
                }
                PollOutcome::Interrupted(v) => {
                    // Ctrl-C: print last-known state to stderr and exit with 130
                    // (conventional SIGINT exit code) so scripts can distinguish
                    // an interrupted poll from a completed one.
                    if v.is_null() {
                        eprintln!("Interrupted before first poll. Transaction id: {pos_txn_id}");
                    } else {
                        let status = pos_status(&v).unwrap_or("unknown");
                        eprintln!("Interrupted. Last known status: {status} (id: {pos_txn_id})");
                    }
                    std::process::exit(130);
                }
            }
            Ok(())
        }
        PosCommand::Get { id } => {
            let (p, api) = build_client(profile)?;
            let result = api.get_pos_transaction(&id).await?;
            render_pos_transaction(&result, output_fmt, &p.name)
        }
        PosCommand::List {
            limit,
            page,
            terminal_id,
        } => {
            let (p, api) = build_client(profile)?;
            let result = api
                .list_pos_transactions(page, Some(limit), terminal_id.as_deref())
                .await?;
            render_pos_transaction_list(&result, output_fmt, &p.name)
        }
        PosCommand::Cancel { id } => {
            let (p, api) = build_client(profile)?;
            let result = api.cancel_pos_transaction(&id).await?;
            render_pos_transaction(&result, output_fmt, &p.name)
        }
    }
}

async fn dispatch_terminals(
    profile: &str,
    output_fmt: cli::OutputFormat,
    tc: cli::TerminalsCommand,
) -> anyhow::Result<()> {
    use cli::TerminalsCommand;
    use cli::terminals::{render_terminal_list, render_terminal_status};

    match tc {
        TerminalsCommand::List { limit, page } => {
            let (p, api) = build_client(profile)?;
            let result = api.list_terminals(page, Some(limit)).await?;
            render_terminal_list(&result, output_fmt, &p.name)
        }
        TerminalsCommand::Status { id } => {
            let (p, api) = build_client(profile)?;
            let result = api.terminal_status(&id).await?;
            render_terminal_status(&result, output_fmt, &p.name)
        }
    }
}

async fn dispatch_devices(
    profile: &str,
    output_fmt: cli::OutputFormat,
    dc: cli::DevicesCommand,
) -> anyhow::Result<()> {
    use cli::DevicesCommand;
    use cli::devices::{
        build_register_device_body, build_ttp_jwt_body, render_device, render_device_list,
        render_ttp_jwt,
    };

    match dc {
        DevicesCommand::List => {
            let (p, api) = build_client(profile)?;
            let result = api.list_devices().await?;
            render_device_list(&result, output_fmt, &p.name)
        }
        DevicesCommand::Get { id } => {
            let (p, api) = build_client(profile)?;
            let result = api.get_device(&id).await?;
            render_device(&result, output_fmt, &p.name)
        }
        DevicesCommand::Register { id, name } => {
            let body = build_register_device_body(&id, name.as_deref());
            let (p, api) = build_client(profile)?;
            let result = api.register_device(body).await?;
            render_device(&result, output_fmt, &p.name)
        }
        DevicesCommand::TtpJwt { device_id } => {
            let body = build_ttp_jwt_body(&device_id);
            let (p, api) = build_client(profile)?;
            let result = api.ttp_jwt(body).await?;
            render_ttp_jwt(&result, output_fmt, &p.name)
        }
        DevicesCommand::TtpActivate { id } => {
            let (p, api) = build_client(profile)?;
            let result = api.ttp_activate(&id).await?;
            render_device(&result, output_fmt, &p.name)
        }
    }
}

async fn dispatch_customers(
    profile: &str,
    output_fmt: cli::OutputFormat,
    cc: cli::CustomersCommand,
) -> anyhow::Result<()> {
    use cli::CustomersCommand;
    use cli::customers::{
        build_add_ach_body, build_add_card_body, build_customer_body, merge_customer_update,
        render_customer, render_customer_list, render_payment_method, render_payment_methods,
    };

    match cc {
        CustomersCommand::Create {
            first_name,
            last_name,
            email,
            company,
            mobile,
        } => {
            let body = build_customer_body(
                first_name.as_deref(),
                last_name.as_deref(),
                company.as_deref(),
                email.as_deref(),
                mobile.as_deref(),
            );
            let (p, api) = build_client(profile)?;
            let result = api.create_customer(body).await?;
            render_customer(&result, output_fmt, &p.name)
        }
        CustomersCommand::Get { id } => {
            let (p, api) = build_client(profile)?;
            let result = api.get_customer(&id).await?;
            render_customer(&result, output_fmt, &p.name)
        }
        CustomersCommand::List {
            limit,
            page,
            search,
        } => {
            let (p, api) = build_client(profile)?;
            let result = api.list_customers(page, Some(limit), search).await?;
            render_customer_list(&result, output_fmt, &p.name)
        }
        CustomersCommand::Update {
            id,
            first_name,
            last_name,
            email,
            company,
            mobile,
        } => {
            // GET-merge-PUT-re-GET pattern:
            // 1. GET current values so omitted flags retain their existing data.
            // 2. Merge user-supplied flags onto the current record.
            // 3. PUT the merged body.  The live API responds 200 with an empty
            //    body, so update_customer returns () — no JSON to decode.
            // 4. GET the customer again (fresh) so we render the server's
            //    canonical post-update state rather than our local merge.
            let (p, api) = build_client(profile)?;
            let current = api.get_customer(&id).await?;
            let body =
                merge_customer_update(&current, first_name, last_name, company, email, mobile);
            api.update_customer(&id, body).await?;
            let fresh = api.get_customer(&id).await?;
            render_customer(&fresh, output_fmt, &p.name)
        }
        CustomersCommand::Delete { id, yes } => {
            if !yes {
                anyhow::bail!(
                    "deletion requires --yes to confirm (e.g. `customers delete {id} --yes`)"
                );
            }
            let (_p, api) = build_client(profile)?;
            treat_404_as_ok(api.delete_customer(&id).await)?;
            match output_fmt {
                cli::OutputFormat::Json => {} // empty stdout, exit 0
                cli::OutputFormat::Table => println!("Deleted customer {id}."),
                cli::OutputFormat::Quiet => println!("{id}"),
            }
            Ok(())
        }
        CustomersCommand::AddCard {
            customer_id,
            card,
            exp,
            cvv,
            name,
        } => {
            let body = build_add_card_body(name.as_deref(), &card, &exp, cvv.as_deref())?;
            let (p, api) = build_client(profile)?;
            let result = api.add_card(&customer_id, body).await?;
            render_payment_method(&result, output_fmt, &p.name)
        }
        CustomersCommand::AddAch {
            customer_id,
            routing,
            account,
            account_type,
            account_holder_type,
            name,
            tax_id,
        } => {
            let body = build_add_ach_body(
                name.as_deref(),
                &account,
                &routing,
                account_type,
                account_holder_type,
                tax_id.as_deref(),
            );
            let (p, api) = build_client(profile)?;
            let result = api.add_ach(&customer_id, body).await?;
            render_payment_method(&result, output_fmt, &p.name)
        }
        CustomersCommand::Methods { customer_id } => {
            let (p, api) = build_client(profile)?;
            let result = api.list_payment_methods(&customer_id).await?;
            render_payment_methods(&result, output_fmt, &p.name)
        }
        CustomersCommand::RemoveMethod {
            customer_id,
            method_id,
            yes,
        } => {
            if !yes {
                anyhow::bail!(
                    "removal requires --yes to confirm (e.g. `customers remove-method {customer_id} {method_id} --yes`)"
                );
            }
            let (_p, api) = build_client(profile)?;
            treat_404_as_ok(api.remove_payment_method(&customer_id, &method_id).await)?;
            match output_fmt {
                cli::OutputFormat::Json => {} // empty stdout, exit 0
                cli::OutputFormat::Table => {
                    println!("Removed payment method {method_id}.")
                }
                cli::OutputFormat::Quiet => println!("{method_id}"),
            }
            Ok(())
        }
    }
}

async fn dispatch_ach(
    profile: &str,
    output_fmt: cli::OutputFormat,
    ac: cli::AchCommand,
) -> anyhow::Result<()> {
    use cli::AchCommand;
    use cli::ach::{AchArgs, AchTxnKind, execute_ach_txn};
    use cli::money::parse_amount;
    use cli::transactions::render_transaction;

    match ac {
        AchCommand::Debit {
            amount,
            payment_processor_id,
            routing,
            account,
            account_type,
            account_holder_type,
            requester_ip,
            sec_code,
            tax_id,
            customer_id,
            payment_method_id,
            faster,
            billing_line1,
            billing_line2,
            billing_city,
            billing_state,
            billing_state_id,
            billing_postal_code,
            billing_country_id,
            contact_first_name,
            contact_last_name,
            contact_email,
            contact_phone,
            contact_company,
        } => {
            let amt = parse_amount(&amount)?;
            execute_ach_txn(
                profile,
                output_fmt,
                AchArgs {
                    amount: amt,
                    payment_processor_id,
                    requester_ip,
                    sec_code,
                    routing,
                    account,
                    account_type,
                    account_holder_type,
                    tax_id,
                    customer_id,
                    payment_method_id,
                    faster,
                    billing_line1,
                    billing_line2,
                    billing_city,
                    billing_state,
                    billing_state_id,
                    billing_postal_code,
                    billing_country_id,
                    contact_first_name,
                    contact_last_name,
                    contact_email,
                    contact_phone,
                    contact_company,
                },
                AchTxnKind::Debit,
            )
            .await
        }
        AchCommand::Credit {
            amount,
            payment_processor_id,
            routing,
            account,
            account_type,
            account_holder_type,
            requester_ip,
            sec_code,
            tax_id,
            customer_id,
            payment_method_id,
            faster,
            billing_line1,
            billing_line2,
            billing_city,
            billing_state,
            billing_state_id,
            billing_postal_code,
            billing_country_id,
            contact_first_name,
            contact_last_name,
            contact_email,
            contact_phone,
            contact_company,
        } => {
            let amt = parse_amount(&amount)?;
            execute_ach_txn(
                profile,
                output_fmt,
                AchArgs {
                    amount: amt,
                    payment_processor_id,
                    requester_ip,
                    sec_code,
                    routing,
                    account,
                    account_type,
                    account_holder_type,
                    tax_id,
                    customer_id,
                    payment_method_id,
                    faster,
                    billing_line1,
                    billing_line2,
                    billing_city,
                    billing_state,
                    billing_state_id,
                    billing_postal_code,
                    billing_country_id,
                    contact_first_name,
                    contact_last_name,
                    contact_email,
                    contact_phone,
                    contact_company,
                },
                AchTxnKind::Credit,
            )
            .await
        }
        AchCommand::Void { id } => {
            let (p, api) = build_client(profile)?;
            let result = api.ach_void(&id).await?;
            render_transaction(&result, output_fmt, &p.name)
        }
        AchCommand::Refund { id } => {
            let (p, api) = build_client(profile)?;
            let result = api.ach_refund(&id).await?;
            render_transaction(&result, output_fmt, &p.name)
        }
    }
}

async fn dispatch_transactions(
    profile: &str,
    output_fmt: cli::OutputFormat,
    tc: cli::TransactionsCommand,
) -> anyhow::Result<()> {
    use cli::TransactionsCommand;
    use cli::money::parse_amount;
    use cli::transactions::{
        CardTxnKind, SaleArgs, build_capture_body, build_refund_body, build_settle_body,
        build_tip_adjust_body, build_void_body, execute_card_txn, filter_items, inspect_table,
        render_list, render_transaction,
    };

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
            let m = parse_txn_money(&amount, tip_amount.as_deref(), l2_tax_rate.as_deref())?;
            execute_card_txn(
                profile,
                output_fmt,
                SaleArgs {
                    amount: m.amount,
                    card,
                    exp,
                    cvv,
                    tip_amount: m.tip_amount,
                    customer_id,
                    payment_method_id,
                    currency_id,
                    card_data_source,
                    l2_tax_rate: m.l2_tax_rate,
                    l3_invoice,
                    l3_po,
                    l3_product,
                    reference_id,
                },
                CardTxnKind::Sale,
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
            let m = parse_txn_money(&amount, tip_amount.as_deref(), l2_tax_rate.as_deref())?;
            execute_card_txn(
                profile,
                output_fmt,
                SaleArgs {
                    amount: m.amount,
                    card,
                    exp,
                    cvv,
                    tip_amount: m.tip_amount,
                    customer_id,
                    payment_method_id,
                    currency_id,
                    card_data_source,
                    l2_tax_rate: m.l2_tax_rate,
                    l3_invoice,
                    l3_po,
                    l3_product,
                    reference_id,
                },
                CardTxnKind::Auth,
            )
            .await
        }
        TransactionsCommand::Capture {
            transaction_id,
            amount,
        } => {
            let amt = amount.as_deref().map(parse_amount).transpose()?;
            let body = build_capture_body(&transaction_id, amt)?;
            let (p, api) = build_client(profile)?;
            let result = api.capture(body).await?;
            render_transaction(&result, output_fmt, &p.name)
        }
        TransactionsCommand::Void { transaction_id } => {
            let body = build_void_body(&transaction_id);
            let (p, api) = build_client(profile)?;
            let result = api.void(body).await?;
            render_transaction(&result, output_fmt, &p.name)
        }
        TransactionsCommand::Refund {
            transaction_id,
            amount,
            card_data_source,
        } => {
            let amt = amount.as_deref().map(parse_amount).transpose()?;
            let body = build_refund_body(&transaction_id, amt, card_data_source)?;
            let (p, api) = build_client(profile)?;
            let result = api.refund(body).await?;
            render_transaction(&result, output_fmt, &p.name)
        }
        TransactionsCommand::Settle {
            payment_processor_id,
        } => {
            let body = build_settle_body(&payment_processor_id);
            let (p, api) = build_client(profile)?;
            let result = api.settle(body).await?;
            render_transaction(&result, output_fmt, &p.name)
        }
        TransactionsCommand::TipAdjust {
            transaction_id,
            tip_amount,
        } => {
            let tip = parse_amount(&tip_amount)?;
            let body = build_tip_adjust_body(&transaction_id, tip)?;
            let (p, api) = build_client(profile)?;
            let result = api.tip_adjust(body).await?;
            render_transaction(&result, output_fmt, &p.name)
        }
        TransactionsCommand::Get { id } => {
            let (p, api) = build_client(profile)?;
            let result = api.get_transaction(&id).await?;
            render_transaction(&result, output_fmt, &p.name)
        }
        TransactionsCommand::List {
            limit,
            page,
            unsettled,
            status,
            from,
            to,
        } => {
            let no_batch = if unsettled { Some(true) } else { None };
            let (p, api) = build_client(profile)?;
            let result = api.list_transactions(page, Some(limit), no_batch).await?;

            let items = result
                .get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let total = result.get("total").and_then(|v| v.as_u64()).unwrap_or(0);

            let filtered = filter_items(&items, status.as_deref(), from.as_deref(), to.as_deref());

            render_list(&filtered, total, output_fmt, &p.name)
        }
        TransactionsCommand::Inspect { id } => {
            let (p, api) = build_client(profile)?;
            let result = api.get_transaction(&id).await?;

            match output_fmt {
                cli::OutputFormat::Json => {
                    let envelope =
                        cli::output::Envelope::new("transaction", result.clone(), &p.name, None);
                    println!("{}", serde_json::to_string_pretty(&envelope)?);
                    Ok(())
                }
                cli::OutputFormat::Table => {
                    println!("{}", inspect_table(&result));
                    Ok(())
                }
                cli::OutputFormat::Quiet => {
                    if let Some(id) = result.get("transactionId").and_then(|v| v.as_str()) {
                        println!("{id}");
                    }
                    Ok(())
                }
            }
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

    // `completion` is offline: run it synchronously before the Tokio runtime
    // so it never needs credentials and produces no async overhead.
    if let cli::Command::Completion { shell } = cmd {
        clap_complete::generate(
            shell,
            &mut cli::Cli::command(),
            "flute",
            &mut std::io::stdout(),
        );
        return Ok(());
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let should_check = should_run_update_check(&cmd, output_fmt);

        let dispatch_result = match cmd {
            // `Completion` is handled above, before runtime creation, so this
            // arm is unreachable by construction. Surface a loud error rather
            // than panicking (no-panic policy) or silently succeeding — if a
            // future refactor routes completion here, this fails visibly
            // instead of emitting nothing and exiting 0.
            cli::Command::Completion { .. } => {
                Err(anyhow::anyhow!("completion is handled before dispatch"))
            }
            cli::Command::Update => update::run().await,
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
            cli::Command::Ach(ac) => dispatch_ach(&profile, output_fmt, *ac).await,
            cli::Command::Customers(cc) => dispatch_customers(&profile, output_fmt, *cc).await,
            cli::Command::Terminals(tc) => dispatch_terminals(&profile, output_fmt, *tc).await,
            cli::Command::Devices(dc) => dispatch_devices(&profile, output_fmt, *dc).await,
            cli::Command::Pos(pc) => dispatch_pos(&profile, output_fmt, *pc).await,
            cli::Command::Settlements(sc) => dispatch_settlements(&profile, output_fmt, *sc).await,
            cli::Command::Tokens(tc) => dispatch_tokens(&profile, output_fmt, *tc).await,
            cli::Command::Subscriptions(sc) => {
                dispatch_subscriptions(&profile, output_fmt, *sc).await
            }
        };

        // Post-dispatch update notice: best-effort, never fails the command.
        // Runs only when all opt-out gates pass and the command itself succeeded.
        if should_check
            && dispatch_result.is_ok()
            && let Some(version) = update_check::check_for_update().await
        {
            eprintln!("\n{}", update_check::notice_for(&version));
        }

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

/// Apply all the opt-out gates for the startup update check.
///
/// The post-dispatch fetch uses this predicate so the rules stay in one place:
/// * Skip for `update` and `auth` (tight single-purpose tasks; banner adds noise).
/// * Skip for `completion` (offline, no TTY implied).
/// * Skip when stderr isn't a TTY (piped output, CI logs).
/// * Skip when `--output json` is set (keeps stdout clean for machine consumers).
/// * Respect the config-file and env-var opt-outs via `update_check::opt_out`.
fn should_run_update_check(cmd: &cli::Command, output_fmt: cli::OutputFormat) -> bool {
    if matches!(
        cmd,
        cli::Command::Update | cli::Command::Auth(_) | cli::Command::Completion { .. }
    ) {
        return false;
    }
    if !std::io::stderr().is_terminal() {
        return false;
    }
    if output_fmt == cli::OutputFormat::Json {
        return false;
    }
    let cfg = config::load_or_default();
    !update_check::opt_out(&cfg)
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

    // ── treat_404_as_ok ──────────────────────────────────────────────────────

    #[test]
    fn treat_404_as_ok_maps_404_to_success() {
        use crate::api::ApiError;
        let err = ApiError::Api {
            status: 404,
            correlation_id: None,
            message: "not found".into(),
        };
        let result = treat_404_as_ok(Err(err));
        assert!(result.is_ok(), "404 must be mapped to Ok");
    }

    #[test]
    fn treat_404_as_ok_passes_through_other_errors() {
        use crate::api::ApiError;
        let err = ApiError::Api {
            status: 500,
            correlation_id: None,
            message: "server error".into(),
        };
        let result = treat_404_as_ok(Err(err));
        assert!(result.is_err(), "non-404 errors must propagate");
    }

    #[test]
    fn treat_404_as_ok_keeps_ok_as_ok() {
        let result = treat_404_as_ok(Ok(()));
        assert!(result.is_ok(), "Ok(()) must remain Ok");
    }

    #[test]
    fn treat_404_as_ok_does_not_swallow_400() {
        use crate::api::ApiError;
        let err = ApiError::Api {
            status: 400,
            correlation_id: None,
            message: "bad request".into(),
        };
        let result = treat_404_as_ok(Err(err));
        assert!(result.is_err(), "400 must not be swallowed");
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

    // ── parse_txn_money ──────────────────────────────────────────────────────

    #[test]
    fn parse_txn_money_parses_all_three_fields() {
        // All three strings use `parse_amount` (≤ 2 decimal places), so the L2
        // rate here is a plain 2-dp value.
        let m = parse_txn_money("10.00", Some("2.50"), Some("8.25")).unwrap();
        assert_eq!(m.amount.to_string(), "10.00");
        assert_eq!(m.tip_amount.unwrap().to_string(), "2.50");
        assert_eq!(m.l2_tax_rate.unwrap().to_string(), "8.25");
    }

    #[test]
    fn parse_txn_money_leaves_optional_fields_none() {
        let m = parse_txn_money("10.00", None, None).unwrap();
        assert_eq!(m.amount.to_string(), "10.00");
        assert!(m.tip_amount.is_none());
        assert!(m.l2_tax_rate.is_none());
    }

    #[test]
    fn parse_txn_money_rejects_invalid_amount() {
        assert!(parse_txn_money("notanumber", None, None).is_err());
    }

    #[test]
    fn parse_txn_money_propagates_invalid_tip() {
        // Negative tip is rejected by `parse_amount` and must bubble up.
        assert!(parse_txn_money("10.00", Some("-1.00"), None).is_err());
    }

    #[test]
    fn parse_txn_money_propagates_invalid_l2_rate() {
        // Scale > 2 is rejected by `parse_amount` and must bubble up.
        assert!(parse_txn_money("10.00", None, Some("1.005")).is_err());
    }
}
