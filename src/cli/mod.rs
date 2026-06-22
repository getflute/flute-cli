//! Clap argument tree for the `flute` binary.

use clap::{Parser, Subcommand};
pub use clap_complete::Shell;

pub mod ach;
pub mod auth;
pub mod customers;
pub mod devices;
pub mod money;
pub mod output;
pub mod pos;
pub mod settlements;
pub mod subscriptions;
pub mod terminals;
pub mod tokens;
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
    /// When omitted, falls back to the `FLUTE_OUTPUT` env var, then to the
    /// `output` key in ~/.flute/config.toml, then to `table`.
    #[arg(long, global = true, value_enum, env = "FLUTE_OUTPUT")]
    pub output: Option<OutputFormat>,

    /// Print HTTP request/response traces to stderr. Card and bank-account
    /// numbers are masked to the last 4 digits; CVV/security codes are removed.
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
    /// ACH payment operations (debit, credit, void, refund).
    #[command(subcommand)]
    Ach(Box<AchCommand>),
    /// Customer and payment-method vault operations.
    #[command(subcommand)]
    Customers(Box<CustomersCommand>),
    /// Terminal operations (list, status).
    #[command(subcommand)]
    Terminals(Box<TerminalsCommand>),
    /// Device operations (list, get, register, ttp-jwt, ttp-activate).
    #[command(subcommand)]
    Devices(Box<DevicesCommand>),
    /// POS transaction operations (create [--wait], get, list, cancel).
    #[command(subcommand)]
    Pos(Box<PosCommand>),
    /// Settlement batch operations (list, get).
    #[command(subcommand)]
    Settlements(Box<SettlementsCommand>),
    /// ISV API token operations (create, list, revoke).
    #[command(subcommand)]
    Tokens(Box<TokensCommand>),
    /// Subscription operations (create, get, list, payments, terminate).
    #[command(subcommand)]
    Subscriptions(Box<SubscriptionsCommand>),
    /// Print shell completion script for the given shell.
    Completion {
        /// Shell to generate completions for.
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Update the flute CLI to the latest release.
    Update,
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

/// Transaction subcommands — Phase 1 lifecycle ops.
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

        /// Currency ID. The API requires this; defaults to 1 (USD). Override for
        /// other currencies.
        #[arg(long, default_value = "1")]
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

        /// Currency ID. The API requires this; defaults to 1 (USD). Override for
        /// other currencies.
        #[arg(long, default_value = "1")]
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

    /// Capture a previously authorised transaction (POST /pay-api/v1/transactions/capture).
    Capture {
        /// UUID of the transaction to capture (required).
        #[arg(long, required = true)]
        transaction_id: String,

        /// Capture amount for a partial capture. Omit for a full capture.
        #[arg(long)]
        amount: Option<String>,
    },

    /// Void a transaction (POST /pay-api/v1/transactions/void).
    Void {
        /// UUID of the transaction to void (required).
        #[arg(long, required = true)]
        transaction_id: String,
    },

    /// Refund (return) a transaction (POST /pay-api/v1/transactions/return).
    Refund {
        /// UUID of the transaction to refund (required).
        #[arg(long, required = true)]
        transaction_id: String,

        /// Refund amount for a partial refund. Omit for a full refund.
        #[arg(long)]
        amount: Option<String>,

        /// Card data source enum (default 1 = Internet/ISV API). Required by the API.
        #[arg(long, default_value_t = 1)]
        card_data_source: i32,
    },

    /// Settle the open batch for a payment processor (POST /pay-api/v1/transactions/settle).
    ///
    /// IMPORTANT: This is a **batch-level** operation, not a per-transaction operation.
    /// The API's SettleRequestDto requires a `paymentProcessorId`; it closes/settles the
    /// processor's entire open batch. Use `--payment-processor-id` to identify the processor.
    Settle {
        /// UUID of the payment processor whose open batch should be settled (required).
        #[arg(long, required = true)]
        payment_processor_id: String,
    },

    /// Adjust the tip on a transaction (POST /pay-api/v1/transactions/tip-adjustment).
    TipAdjust {
        /// UUID of the transaction to adjust (required).
        #[arg(long, required = true)]
        transaction_id: String,

        /// New tip amount (required). Plain decimal, e.g. `3.50`.
        #[arg(long, required = true)]
        tip_amount: String,
    },

    /// Fetch a single transaction by ID (GET /pay-api/v1/transactions/{id}).
    Get {
        /// Transaction UUID to retrieve (positional).
        id: String,
    },

    /// List transactions for the current merchant (GET /pay-api/v1/transactions).
    ///
    /// Fetches a page of transactions and optionally filters client-side.
    /// **Note**: `--status`, `--from`, and `--to` are applied locally to the
    /// returned page only — they do NOT send additional server-side query params.
    /// Use `--limit` and `--page` to control which page is fetched.
    List {
        /// Maximum results per page (default 25). Maps to `pageSize` on the API.
        #[arg(long, default_value_t = 25)]
        limit: u32,

        /// Page number to fetch (1-based). Maps to `page` on the API.
        #[arg(long)]
        page: Option<u32>,

        /// Fetch only unsettled (no-batch) transactions. Maps to `noBatch=true`.
        #[arg(long)]
        unsettled: bool,

        /// Filter results by status (case-insensitive, client-side, current page only).
        #[arg(long)]
        status: Option<String>,

        /// Filter results from this date inclusive (YYYY-MM-DD, client-side, current page only).
        #[arg(long)]
        from: Option<String>,

        /// Filter results up to this date inclusive (YYYY-MM-DD, client-side, current page only).
        #[arg(long)]
        to: Option<String>,
    },

    /// Show rich details for a single transaction (GET /pay-api/v1/transactions/{id}).
    ///
    /// Displays all key fields including the amount breakdown and the
    /// `availableOperations` list from the API response — no client-side derivation.
    Inspect {
        /// Transaction UUID to inspect (positional).
        id: String,
    },
}

/// ACH subcommands — Phase 2 lifecycle ops.
#[derive(Subcommand, Debug)]
pub enum AchCommand {
    /// ACH debit payment (POST /pay-api/v1/transactions/ach/payment).
    ///
    /// Live API requires `--account-holder-type`, `--billing-line1` (and at least
    /// one other billing field), and `--contact-first-name`/`--contact-last-name`
    /// beyond the OpenAPI "required" list — include them to avoid 422 rejections.
    Debit {
        /// Transaction amount (required). Plain decimal, e.g. `500.00`.
        #[arg(long, required = true)]
        amount: String,

        /// Payment processor UUID (required).
        #[arg(long, required = true)]
        payment_processor_id: String,

        /// Routing number (ABA).
        #[arg(long)]
        routing: Option<String>,

        /// Bank account number.
        #[arg(long)]
        account: Option<String>,

        /// Account type: `checking` (default) or `savings`.
        #[arg(long, value_enum)]
        account_type: Option<ach::AccountTypeArg>,

        /// Account holder type: `business` or `personal`. Live-required for debit/credit.
        #[arg(long, value_enum)]
        account_holder_type: Option<ach::AccountHolderTypeArg>,

        /// End-customer IP address. Default `127.0.0.1`.
        #[arg(long, default_value = "127.0.0.1")]
        requester_ip: String,

        /// ACH SEC code integer (default 1 = Web). Values: 1=Web, 2=PPD, 3=CCD, 4=Telephone.
        #[arg(long, default_value_t = 1)]
        sec_code: i32,

        /// Tax ID (optional).
        #[arg(long)]
        tax_id: Option<String>,

        /// Customer UUID for vault-linked ACH.
        #[arg(long)]
        customer_id: Option<String>,

        /// Payment method UUID (stored ACH account).
        #[arg(long)]
        payment_method_id: Option<String>,

        /// Enable faster processing (default false).
        #[arg(long, default_value_t = false)]
        faster: bool,

        // ── Billing address (live-required) ───────────────────────────────────
        /// Billing address line 1.
        #[arg(long)]
        billing_line1: Option<String>,

        /// Billing address line 2.
        #[arg(long)]
        billing_line2: Option<String>,

        /// Billing city.
        #[arg(long)]
        billing_city: Option<String>,

        /// Billing state name (e.g. `IL`).
        #[arg(long)]
        billing_state: Option<String>,

        /// Billing state ID (integer). The live API requires a numeric stateId; use
        /// alongside `--billing-country-id 1` for US addresses.
        #[arg(long)]
        billing_state_id: Option<i32>,

        /// Billing postal/ZIP code.
        #[arg(long)]
        billing_postal_code: Option<String>,

        /// Billing country ID (integer; US = 1). Required alongside --billing-state-id for live calls.
        #[arg(long)]
        billing_country_id: Option<i32>,

        // ── Contact info (live-required) ──────────────────────────────────────
        /// Contact first name.
        #[arg(long)]
        contact_first_name: Option<String>,

        /// Contact last name.
        #[arg(long)]
        contact_last_name: Option<String>,

        /// Contact email address.
        #[arg(long)]
        contact_email: Option<String>,

        /// Contact mobile/phone number.
        #[arg(long)]
        contact_phone: Option<String>,

        /// Contact company name.
        #[arg(long)]
        contact_company: Option<String>,
    },

    /// ACH credit payment (POST /pay-api/v1/transactions/ach/payment/credit).
    ///
    /// Live API requires `--account-holder-type`, `--billing-*`, and `--contact-*`
    /// fields beyond the OpenAPI "required" list — include them to avoid 422 rejections.
    Credit {
        /// Transaction amount (required). Plain decimal, e.g. `500.00`.
        #[arg(long, required = true)]
        amount: String,

        /// Payment processor UUID (required).
        #[arg(long, required = true)]
        payment_processor_id: String,

        /// Routing number (ABA).
        #[arg(long)]
        routing: Option<String>,

        /// Bank account number.
        #[arg(long)]
        account: Option<String>,

        /// Account type: `checking` (default) or `savings`.
        #[arg(long, value_enum)]
        account_type: Option<ach::AccountTypeArg>,

        /// Account holder type: `business` or `personal`. Live-required for debit/credit.
        #[arg(long, value_enum)]
        account_holder_type: Option<ach::AccountHolderTypeArg>,

        /// End-customer IP address. Default `127.0.0.1`.
        #[arg(long, default_value = "127.0.0.1")]
        requester_ip: String,

        /// ACH SEC code integer (default 1 = Web). Values: 1=Web, 2=PPD, 3=CCD, 4=Telephone.
        #[arg(long, default_value_t = 1)]
        sec_code: i32,

        /// Tax ID (optional).
        #[arg(long)]
        tax_id: Option<String>,

        /// Customer UUID for vault-linked ACH.
        #[arg(long)]
        customer_id: Option<String>,

        /// Payment method UUID (stored ACH account).
        #[arg(long)]
        payment_method_id: Option<String>,

        /// Enable faster processing (default false).
        #[arg(long, default_value_t = false)]
        faster: bool,

        // ── Billing address (live-required) ───────────────────────────────────
        /// Billing address line 1.
        #[arg(long)]
        billing_line1: Option<String>,

        /// Billing address line 2.
        #[arg(long)]
        billing_line2: Option<String>,

        /// Billing city.
        #[arg(long)]
        billing_city: Option<String>,

        /// Billing state name (e.g. `IL`).
        #[arg(long)]
        billing_state: Option<String>,

        /// Billing state ID (integer). The live API requires a numeric stateId; use
        /// alongside `--billing-country-id 1` for US addresses.
        #[arg(long)]
        billing_state_id: Option<i32>,

        /// Billing postal/ZIP code.
        #[arg(long)]
        billing_postal_code: Option<String>,

        /// Billing country ID (integer; US = 1). Required alongside --billing-state-id for live calls.
        #[arg(long)]
        billing_country_id: Option<i32>,

        // ── Contact info (live-required) ──────────────────────────────────────
        /// Contact first name.
        #[arg(long)]
        contact_first_name: Option<String>,

        /// Contact last name.
        #[arg(long)]
        contact_last_name: Option<String>,

        /// Contact email address.
        #[arg(long)]
        contact_email: Option<String>,

        /// Contact mobile/phone number.
        #[arg(long)]
        contact_phone: Option<String>,

        /// Contact company name.
        #[arg(long)]
        contact_company: Option<String>,
    },

    /// Void an ACH transaction by ID (POST /pay-api/v1/transactions/ach/{id}/void).
    Void {
        /// ACH transaction UUID to void (positional).
        id: String,
    },

    /// Refund an ACH transaction by ID (POST /pay-api/v1/transactions/ach/{id}/refund).
    Refund {
        /// ACH transaction UUID to refund (positional).
        id: String,
    },
}

/// Customers / Vault subcommands — Phase 2 Task 2.2.
#[derive(Subcommand, Debug)]
pub enum CustomersCommand {
    /// Create a new customer (POST /pay-api/v1/customers).
    Create {
        /// Customer first name.
        #[arg(long)]
        first_name: Option<String>,

        /// Customer last name.
        #[arg(long)]
        last_name: Option<String>,

        /// Customer email address.
        #[arg(long)]
        email: Option<String>,

        /// Customer company name.
        #[arg(long)]
        company: Option<String>,

        /// Customer mobile phone number.
        #[arg(long)]
        mobile: Option<String>,
    },

    /// Fetch a single customer by ID (GET /pay-api/v1/customers/{id}).
    Get {
        /// Customer UUID to retrieve (positional).
        id: String,
    },

    /// List customers (GET /pay-api/v1/customers).
    List {
        /// Maximum results per page (default 25). Maps to `pageSize` on the API.
        #[arg(long, default_value_t = 25)]
        limit: u32,

        /// Page number to fetch (1-based). Maps to `page` on the API.
        #[arg(long)]
        page: Option<u32>,

        /// Server-side text search across name/email.
        #[arg(long)]
        search: Option<String>,
    },

    /// Update a customer (PUT /pay-api/v1/customers/{id}).
    ///
    /// The API treats PUT as a full replacement, so the CLI does GET-merge-PUT:
    /// it fetches the current record, overlays the flags you supply, and PUTs the
    /// merged result. Omitted flags retain their existing server values.
    Update {
        /// Customer UUID to update (positional).
        id: String,

        /// New first name.
        #[arg(long)]
        first_name: Option<String>,

        /// New last name.
        #[arg(long)]
        last_name: Option<String>,

        /// New email address.
        #[arg(long)]
        email: Option<String>,

        /// New company name.
        #[arg(long)]
        company: Option<String>,

        /// New mobile phone number.
        #[arg(long)]
        mobile: Option<String>,
    },

    /// Delete a customer (DELETE /pay-api/v1/customers/{id}).
    ///
    /// Requires `--yes` to prevent accidental deletions.
    Delete {
        /// Customer UUID to delete (positional).
        id: String,

        /// Confirm the deletion (required).
        #[arg(long)]
        yes: bool,
    },

    /// Vault a card for a customer (POST /pay-api/v1/customers/{id}/payment-methods/cards).
    AddCard {
        /// Customer UUID (positional).
        customer_id: String,

        /// Card PAN (primary account number), e.g. `4111111111111111`.
        #[arg(long, required = true)]
        card: String,

        /// Card expiry in MM/YY or MM/YYYY format, e.g. `12/26`.
        #[arg(long, required = true)]
        exp: String,

        /// Card CVV/security code.
        #[arg(long)]
        cvv: Option<String>,

        /// Friendly label for this payment method.
        #[arg(long)]
        name: Option<String>,
    },

    /// Vault an ACH account for a customer (POST /pay-api/v1/customers/{id}/payment-methods/ach).
    AddAch {
        /// Customer UUID (positional).
        customer_id: String,

        /// ABA routing number.
        #[arg(long, required = true)]
        routing: String,

        /// Bank account number.
        #[arg(long, required = true)]
        account: String,

        /// Account type: `checking` (default) or `savings`.
        #[arg(long, value_enum, default_value = "checking")]
        account_type: ach::AccountTypeArg,

        /// Account holder type: `business` or `personal`. Omit if not applicable.
        #[arg(long, value_enum)]
        account_holder_type: Option<ach::AccountHolderTypeArg>,

        /// Friendly label for this payment method.
        #[arg(long)]
        name: Option<String>,

        /// Tax ID (optional).
        #[arg(long)]
        tax_id: Option<String>,
    },

    /// List payment methods for a customer (GET /pay-api/v1/customers/{id}/payment-methods).
    Methods {
        /// Customer UUID (positional).
        customer_id: String,
    },

    /// Remove a payment method from a customer's vault.
    ///
    /// DELETE /pay-api/v1/customers/{id}/payment-methods/{mid}.
    /// Requires `--yes` to prevent accidental removal.
    RemoveMethod {
        /// Customer UUID (positional).
        customer_id: String,

        /// Payment method UUID (positional).
        method_id: String,

        /// Confirm the removal (required).
        #[arg(long)]
        yes: bool,
    },
}

/// Terminals subcommands — Phase 3 Task 3.1.
#[derive(Subcommand, Debug)]
pub enum TerminalsCommand {
    /// List terminals (GET /pos-api/v1/terminals).
    List {
        /// Maximum results per page (default 25). Maps to `pageSize` on the API.
        #[arg(long, default_value_t = 25)]
        limit: u32,

        /// Page number to fetch (1-based). Maps to `page` on the API.
        #[arg(long)]
        page: Option<u32>,
    },

    /// Retrieve terminal status by ID (GET /pos-api/v1/terminals/{id}/status).
    Status {
        /// Terminal UUID (positional).
        id: String,
    },
}

/// Devices subcommands — Phase 3 Task 3.1.
#[derive(Subcommand, Debug)]
pub enum DevicesCommand {
    /// List all devices (GET /pay-api/v1/devices).
    List,

    /// Fetch a single device by ID (GET /pay-api/v1/devices/{id}).
    Get {
        /// Device ID (positional).
        id: String,
    },

    /// Register (create or update) a device (POST /pay-api/v1/devices).
    Register {
        /// Device ID (positional). Maps to `deviceId` in the request body.
        id: String,

        /// Friendly name for this device. Maps to `deviceName` (optional).
        #[arg(long)]
        name: Option<String>,
    },

    /// Generate a Tap-to-Pay JWT for a device (POST /pay-api/v1/devices/tap-to-pay/jwt).
    TtpJwt {
        /// Device ID to generate JWT for (required). Maps to `deviceId` in body.
        #[arg(long, required = true)]
        device_id: String,
    },

    /// Activate Tap-to-Pay for a device (POST /pay-api/v1/devices/{id}/tap-to-pay/activate).
    ///
    /// This is a bodyless POST — no request body is sent.
    TtpActivate {
        /// Device ID (positional).
        id: String,
    },
}

/// POS transaction subcommands — Phase 3 Task 3.2.
// PosCommand::Create carries many optional String fields (all the POS request
// fields). The enum is always heap-allocated via `Box<PosCommand>` in `Command`,
// so the stack impact is a single pointer. The lint would push us towards nested
// boxing of individual fields without benefit.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand, Debug)]
pub enum PosCommand {
    /// Create a POS transaction (POST /pos-api/v1/pos-transactions).
    ///
    /// Use `--wait` to long-poll until the terminal completes or rejects the
    /// transaction.  Ctrl-C gracefully interrupts the poll and prints the
    /// last-known status.
    Create {
        /// Terminal UUID to send the transaction to (required).
        #[arg(long, required = true)]
        terminal_id: String,

        /// Transaction amount. Plain decimal, e.g. `100.00`. Required for Sale/Auth.
        #[arg(long)]
        amount: Option<String>,

        /// Transaction type ID: 1=Authorization, 2=Sale (default), 3=Capture, 4=Void, 5=Refund.
        #[arg(long, default_value_t = 2)]
        transaction_type: i32,

        /// Currency ID (default 1 = USD).
        #[arg(long, default_value_t = 1)]
        currency_id: i32,

        /// Tip amount. Plain decimal, e.g. `5.00`.
        #[arg(long)]
        tip_amount: Option<String>,

        /// Tip rate as a decimal fraction, e.g. `0.18` for 18%.
        #[arg(long)]
        tip_rate: Option<String>,

        /// POS device ID (required). The API rejects creates without this field.
        #[arg(long, required = true)]
        pos_device_id: Option<String>,

        /// Merchant-assigned reference ID for idempotency tracking (required).
        /// The API rejects creates without this field.
        #[arg(long, required = true)]
        reference_id: Option<String>,

        /// Payment processor UUID.
        #[arg(long)]
        payment_processor_id: Option<String>,

        /// Customer UUID for vault-linked transactions.
        #[arg(long)]
        customer_id: Option<String>,

        /// Target transaction UUID (required for Void/Capture/Refund types).
        #[arg(long)]
        target_transaction_id: Option<String>,

        /// Reading method ID: 1=Reading (default), 2=KeyedIn. Omit to let the server decide.
        #[arg(long)]
        reading_method: Option<i32>,

        /// Long-poll for terminal acceptance: set `waitForAcceptanceByTerminal=true` and
        /// poll GET /pos-transactions/{id} every 2 seconds until isCompleted or timeout.
        #[arg(long)]
        wait: bool,

        /// Seconds to wait before giving up the poll (default 120). Requires `--wait`.
        #[arg(long, default_value_t = 120)]
        wait_timeout: u64,
    },

    /// Fetch a single POS transaction by ID (GET /pos-api/v1/pos-transactions/{id}).
    Get {
        /// POS transaction UUID to retrieve (positional).
        id: String,
    },

    /// List POS transactions (GET /pos-api/v1/pos-transactions).
    List {
        /// Maximum results per page (default 25). Maps to `pageSize` on the API.
        #[arg(long, default_value_t = 25)]
        limit: u32,

        /// Page number to fetch (1-based). Maps to `page` on the API.
        #[arg(long)]
        page: Option<u32>,

        /// Filter by terminal UUID. Maps to `terminalId` on the API.
        #[arg(long)]
        terminal_id: Option<String>,
    },

    /// Cancel a POS transaction (POST /pos-api/v1/pos-transactions/{id}/cancel).
    ///
    /// This is a bodyless POST — no request body is sent.
    Cancel {
        /// POS transaction UUID to cancel (positional).
        id: String,
    },
}

/// Settlements subcommands — Phase 4 Task 4.1.
#[derive(Subcommand, Debug)]
pub enum SettlementsCommand {
    /// List settlement batches (GET /pay-api/v1/settlements/batches).
    List {
        /// Maximum results per page (default 25). Maps to `pageSize` on the API.
        #[arg(long, default_value_t = 25)]
        limit: u32,

        /// Page number to fetch (1-based). Maps to `page` on the API.
        #[arg(long)]
        page: Option<u32>,

        /// Filter from this date inclusive (YYYY-MM-DD). Maps to `dateFrom`.
        #[arg(long)]
        from: Option<String>,

        /// Filter up to this date inclusive (YYYY-MM-DD). Maps to `dateTo`.
        #[arg(long)]
        to: Option<String>,

        /// Filter by status: `open` (statusId=1) or `settled` (statusId=2).
        #[arg(long)]
        status: Option<String>,
    },

    /// Fetch a single settlement batch by ID.
    ///
    /// **Note**: There is no single-batch endpoint. This fetches a page of up to
    /// 100 batches and filters client-side by `id`. Results are page-bounded.
    Get {
        /// Settlement batch ID (positional).
        id: String,
    },
}

/// ISV Tokens subcommands — Phase 4 Task 4.1.
#[derive(Subcommand, Debug)]
pub enum TokensCommand {
    /// Create an ISV API token (POST /pay-api/v1/merchants/tokens).
    ///
    /// The response contains `clientSecret` which is shown **only once**.
    /// Store it securely immediately after creation.
    Create {
        /// Merchant UUID (required). Maps to `merchantId` in the request body.
        #[arg(long, required = true)]
        merchant_id: String,

        /// Token display name (required). Maps to `tokenName` in the request body.
        #[arg(long, required = true)]
        name: String,
    },

    /// List ISV API tokens (GET /pay-api/v1/merchants/tokens).
    List {
        /// Filter by merchant UUID. Maps to `merchantId` query param (optional).
        #[arg(long)]
        merchant_id: Option<String>,
    },

    /// Revoke an ISV API token (DELETE /pay-api/v1/merchants/tokens/{clientId}?merchantId=).
    ///
    /// Both `--client-id` and `--merchant-id` are required by the API.
    /// Requires `--yes` to prevent accidental revocation.
    /// 404 is treated as idempotent success (already revoked).
    Revoke {
        /// Client ID of the token to revoke (required).
        #[arg(long, required = true)]
        client_id: String,

        /// Merchant UUID that owns the token (required). The API returns 400 without it.
        #[arg(long, required = true)]
        merchant_id: String,

        /// Confirm the revocation (required).
        #[arg(long)]
        yes: bool,
    },
}

/// Subscriptions subcommands — Phase 4 Task 4.2.
#[derive(Subcommand, Debug)]
pub enum SubscriptionsCommand {
    /// Create a new subscription (POST /sub-api/v1/subscriptions).
    Create {
        /// Customer UUID (required). Maps to `customerId`.
        #[arg(long, required = true)]
        customer_id: String,

        /// Payment method UUID for the subscription (required). Maps to `paymentMethodId`.
        /// Must be a vaulted and active payment method.
        #[arg(long, required = true)]
        payment_method_id: String,

        /// Subscription charge amount (required). Plain decimal, e.g. `49.99`.
        #[arg(long, required = true)]
        amount: String,

        /// Currency ID (default 1 = USD). Maps to `currencyId`.
        #[arg(long, default_value_t = 1)]
        currency_id: i32,

        /// Total number of scheduled payments (required). Maps to `numberOfPayments`.
        #[arg(long, required = true)]
        number_of_payments: u32,

        /// Payment frequency (default 1 = every 1 unit). Maps to `paymentFrequency`.
        #[arg(long, default_value_t = 1)]
        payment_frequency: u32,

        /// Payment interval: `day`, `week`, or `month` (default `month`).
        /// Maps to `paymentFrequencyUnit` (1=Day, 2=Week, 3=Month).
        #[arg(long, value_enum, default_value = "month")]
        interval: subscriptions::Interval,

        /// Transaction type (default 2 = Sale). Maps to `transactionType`.
        /// 1=Authorization, 2=Sale, 11=AchDebit.
        #[arg(long, default_value_t = 2)]
        transaction_type: i32,

        /// End-customer IP address (default `127.0.0.1`). Maps to `requesterIpAddress`.
        #[arg(long, default_value = "127.0.0.1")]
        requester_ip: String,

        /// Payment processor UUID. Optional. Maps to `paymentProcessorId`.
        #[arg(long)]
        payment_processor_id: Option<String>,

        /// Subscription start date-time (ISO 8601). Omit for immediate/PayNow.
        /// Maps to `paymentStartDateTime`.
        #[arg(long)]
        start_date: Option<String>,

        /// ACH SEC code integer (for ACH subscriptions only). Maps to `secCode`.
        #[arg(long)]
        sec_code: Option<i32>,

        /// Enable faster processing (ACH subscriptions only). Maps to `isFasterProcessing`.
        #[arg(long)]
        faster: bool,
    },

    /// Fetch a single subscription by ID (GET /sub-api/v1/subscriptions/{id}).
    Get {
        /// Subscription UUID (positional).
        id: String,
    },

    /// List subscriptions (GET /sub-api/v1/subscriptions).
    List {
        /// Maximum results per page (default 25). Maps to `pageSize` on the API.
        #[arg(long, default_value_t = 25)]
        limit: u32,

        /// Page number to fetch (1-based). Maps to `page` on the API.
        #[arg(long)]
        page: Option<u32>,

        /// Server-side text search. Maps to `search` on the API.
        #[arg(long)]
        search: Option<String>,

        /// Filter by customer UUID. Maps to `customerIds` on the API.
        #[arg(long)]
        customer_id: Option<String>,

        /// Filter results by subscription status (e.g. `active`, `paused`, `terminated`).
        /// Applied client-side to the returned page only — does NOT send a server query param.
        #[arg(long)]
        status: Option<String>,
    },

    /// List payments for a subscription (GET /sub-api/v1/subscriptions/{id}/payments).
    Payments {
        /// Subscription UUID (positional).
        id: String,
    },

    /// Terminate a subscription (PUT /sub-api/v1/subscriptions/{id}/terminate).
    ///
    /// Requires `--yes` to prevent accidental termination.
    Terminate {
        /// Subscription UUID (positional).
        id: String,

        /// Confirm the termination (required).
        #[arg(long)]
        yes: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use clap::Parser;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn transactions_auth_currency_id_defaults_to_1() {
        let cli = Cli::try_parse_from(["flute", "transactions", "auth", "--amount", "1.00"])
            .expect("parse must succeed");
        match cli.command {
            Some(Command::Transactions(cmd)) => match *cmd {
                TransactionsCommand::Auth { currency_id, .. } => {
                    assert_eq!(
                        currency_id,
                        Some(1),
                        "--currency-id must default to Some(1) for auth"
                    );
                }
                other => panic!("expected Auth variant, got {other:?}"),
            },
            other => panic!("expected Transactions command, got {other:?}"),
        }
    }
}
