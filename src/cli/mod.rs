//! Clap argument tree for the `flute` binary.

use clap::{Parser, Subcommand};

pub mod auth;
pub mod money;
pub mod output;
pub mod transactions;
pub mod util;

pub use output::OutputFormat;

#[derive(Parser, Debug)]
#[command(name = "flute", version, about = "CLI for the Flute payments platform")]
pub struct Cli {
    /// Active profile (environment). `sandbox` (default) or `production`/`prod`.
    #[arg(long, env = "FLUTE_PROFILE", default_value = "sandbox", global = true)]
    pub profile: String,

    /// Output format: table (default), json, or quiet (id only).
    /// When omitted, falls back to the `output` key in ~/.flute/config.toml,
    /// then to `table`.
    #[arg(long, global = true, value_enum)]
    pub output: Option<OutputFormat>,

    /// ISV merchant context for commands whose endpoints accept it.
    #[arg(long, global = true)]
    pub merchant_id: Option<String>,

    /// Print full HTTP request/response (sensitive fields redacted) to stderr.
    #[arg(long, global = true)]
    pub debug: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Authentication and profile management.
    #[command(subcommand)]
    Auth(AuthCommand),
    /// API health check.
    Ping,
    /// Print CLI version and active profile.
    Version,
    /// Transaction operations (sale, auth, …).
    #[command(subcommand)]
    Transactions(Box<TransactionsCommand>),
}

#[derive(Subcommand, Debug)]
pub enum AuthCommand {
    /// Prompt for client_id + client_secret and store them in the OS keychain.
    Login,
    /// Show active profile, environment, and token status.
    Status,
    /// Set the default profile in ~/.flute/config.toml.
    Switch { profile: String },
    /// Clear stored credentials for the active profile.
    Logout,
    /// Print the current bearer token (debugging aid).
    Token,
}

/// Transaction subcommands — Phase 1 exposes `sale` and `auth` only.
/// Capture/void/refund/settle/tip-adjust/get/list/inspect are added in later
/// units and are intentionally absent here (YAGNI).
#[derive(Subcommand, Debug)]
pub enum TransactionsCommand {
    /// Charge a card immediately (POST /pay-api/v1/transactions/sale).
    Sale {
        /// Transaction amount (required). Plain decimal, e.g. `100.00`.
        #[arg(long, required = true)]
        amount: String,

        /// Card PAN (primary account number), e.g. `4111111111111111`.
        #[arg(long)]
        card: Option<String>,

        /// Card expiry in MM/YY or MM/YYYY format, e.g. `12/26`.
        #[arg(long)]
        exp: Option<String>,

        /// Card CVV/security code.
        #[arg(long)]
        cvv: Option<String>,

        /// Tip amount to add to the transaction.
        #[arg(long)]
        tip_amount: Option<String>,

        /// Customer UUID for vault-linked charges.
        #[arg(long)]
        customer_id: Option<String>,

        /// Payment method UUID (stored card) to charge instead of raw PAN.
        #[arg(long)]
        payment_method_id: Option<String>,

        /// Currency ID (server defaults when absent; do NOT pass 0).
        #[arg(long)]
        currency_id: Option<i32>,

        /// Card data source enum (default 1 = Internet/ISV API).
        #[arg(long, default_value_t = 1)]
        card_data_source: i32,

        /// Level-2 sales tax rate, e.g. `0.08` for 8%.
        #[arg(long)]
        l2_tax_rate: Option<String>,

        /// Level-3 invoice number.
        #[arg(long)]
        l3_invoice: Option<String>,

        /// Level-3 purchase order number.
        #[arg(long)]
        l3_po: Option<String>,

        /// Level-3 product line item. Format: `Description,SKU,UnitPrice,UnitOfMeasure,Quantity`.
        /// Repeat the flag for multiple products. Note: the flag itself is repeatable — do NOT
        /// use a comma-delimited list since the product format already uses commas internally.
        #[arg(long, action = clap::ArgAction::Append)]
        l3_product: Vec<String>,

        /// Merchant-assigned reference ID for idempotency tracking.
        #[arg(long)]
        reference_id: Option<String>,
    },

    /// Authorise (hold) a card without capturing (POST /pay-api/v1/transactions/auth).
    Auth {
        /// Transaction amount (required). Plain decimal, e.g. `100.00`.
        #[arg(long, required = true)]
        amount: String,

        /// Card PAN (primary account number).
        #[arg(long)]
        card: Option<String>,

        /// Card expiry in MM/YY or MM/YYYY format.
        #[arg(long)]
        exp: Option<String>,

        /// Card CVV/security code.
        #[arg(long)]
        cvv: Option<String>,

        /// Tip amount.
        #[arg(long)]
        tip_amount: Option<String>,

        /// Customer UUID.
        #[arg(long)]
        customer_id: Option<String>,

        /// Payment method UUID.
        #[arg(long)]
        payment_method_id: Option<String>,

        /// Currency ID (server defaults when absent).
        #[arg(long)]
        currency_id: Option<i32>,

        /// Card data source enum (default 1 = Internet/ISV API).
        #[arg(long, default_value_t = 1)]
        card_data_source: i32,

        /// Level-2 sales tax rate.
        #[arg(long)]
        l2_tax_rate: Option<String>,

        /// Level-3 invoice number.
        #[arg(long)]
        l3_invoice: Option<String>,

        /// Level-3 purchase order number.
        #[arg(long)]
        l3_po: Option<String>,

        /// Level-3 product line item (repeatable).
        #[arg(long, action = clap::ArgAction::Append)]
        l3_product: Vec<String>,

        /// Reference ID.
        #[arg(long)]
        reference_id: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
