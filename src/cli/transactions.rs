//! CLI handlers for the Transactions command group (`flute transactions …`).
//!
//! Populated in Tasks 1.2+; the module is wired here so the module tree
//! compiles from the outset.

use anyhow::Result;
use rust_decimal::Decimal;
use serde_json::{Map, Value, json};

use crate::api::models::to_amount_number;
use crate::cli::output::{Envelope, OutputFormat};

// ── Body-builder ─────────────────────────────────────────────────────────────

/// All CLI flags that feed a `sale` or `auth` request.
///
/// `card_data_source` defaults to `1` (Internet/ISV API).
/// `currency_id` is `None` by default — the server defaults when absent.
pub(crate) struct SaleArgs {
    pub amount: Decimal,
    pub card: Option<String>,
    pub exp: Option<String>,
    pub cvv: Option<String>,
    pub tip_amount: Option<Decimal>,
    pub customer_id: Option<String>,
    pub payment_method_id: Option<String>,
    pub currency_id: Option<i32>,
    /// CardDataSource enum: 1 = Internet/ISV (default). Expose via --card-data-source.
    pub card_data_source: i32,
    pub l2_tax_rate: Option<Decimal>,
    pub l3_invoice: Option<String>,
    pub l3_po: Option<String>,
    /// Each entry is `"Name,SKU,UnitPrice,UnitOfMeasure,Quantity"` (comma-separated).
    /// Multiple --l3-product flags are collected into an array of product objects.
    pub l3_product: Vec<String>,
    pub reference_id: Option<String>,
}

/// Parse a single `--l3-product` token into an object.
///
/// Expected format: `"Description,ProductCode,UnitPrice,UnitOfMeasure,Quantity"`
/// (all comma-separated, positional). Missing trailing fields default to absent
/// in the JSON object. At a minimum the first field (description) is required;
/// the rest are optional and silently absent if empty or not provided.
///
/// # Field mapping (best-effort, position-based)
/// | Pos | Name          | Wire key        |
/// |-----|---------------|-----------------|
/// | 0   | Description   | `description`   |
/// | 1   | Product code  | `productCode`   |
/// | 2   | Unit price    | `unitPrice`     |
/// | 3   | Unit of meas. | `unitOfMeasure` |
/// | 4   | Quantity      | `quantity`      |
fn parse_l3_product(s: &str) -> Value {
    // Reject entirely blank/whitespace input or input with no description
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Value::Null;
    }
    let parts: Vec<&str> = trimmed.splitn(5, ',').collect();
    // description (pos 0) must be non-empty after trimming
    let description = parts.first().map(|v| v.trim()).unwrap_or("");
    if description.is_empty() {
        return Value::Null;
    }
    let mut obj = Map::new();
    // description (pos 0)
    if let Some(v) = parts.first()
        && !v.trim().is_empty()
    {
        obj.insert("description".into(), Value::String(v.trim().to_string()));
    }
    // productCode (pos 1)
    if let Some(v) = parts.get(1)
        && !v.trim().is_empty()
    {
        obj.insert("productCode".into(), Value::String(v.trim().to_string()));
    }
    // unitPrice (pos 2) — try to parse as number
    if let Some(v) = parts.get(2)
        && !v.trim().is_empty()
    {
        if let Ok(d) = v.trim().parse::<Decimal>() {
            obj.insert("unitPrice".into(), to_amount_number(d));
        } else {
            obj.insert("unitPrice".into(), Value::String(v.trim().to_string()));
        }
    }
    // unitOfMeasure (pos 3)
    if let Some(v) = parts.get(3)
        && !v.trim().is_empty()
    {
        obj.insert("unitOfMeasure".into(), Value::String(v.trim().to_string()));
    }
    // quantity (pos 4) — try to parse as number
    if let Some(v) = parts.get(4)
        && !v.trim().is_empty()
    {
        if let Ok(d) = v.trim().parse::<Decimal>() {
            obj.insert("quantity".into(), to_amount_number(d));
        } else {
            obj.insert("quantity".into(), Value::String(v.trim().to_string()));
        }
    }
    Value::Object(obj)
}

/// Build the JSON request body for a `sale` or `auth` request.
///
/// This is a **pure function** — no I/O, no network, trivially unit-testable.
///
/// ## Design notes
/// - Amounts are inserted via `to_amount_number` (arbitrary_precision JSON Number)
///   so the wire format is exact decimal (e.g. `100.00`), not a float artifact.
/// - `currencyId` is **omitted** unless `--currency-id` is passed; the server
///   handles the default.
/// - `cardDataSource` is always present (required by the API spec).
/// - PAN (`accountNumber`) appears in the request body as-is; redaction is only
///   applied to `--debug` log output, not to the wire body.
pub(crate) fn build_sale_body(args: &SaleArgs) -> Result<Value> {
    let mut obj = Map::new();

    // Required-ish fields always present
    obj.insert("amount".into(), to_amount_number(args.amount));
    obj.insert("cardDataSource".into(), json!(args.card_data_source));
    obj.insert("customerInitiatedTransaction".into(), json!(false));

    // Optional card fields
    if let Some(card) = &args.card {
        obj.insert("accountNumber".into(), Value::String(card.clone()));
    }
    if let Some(cvv) = &args.cvv {
        obj.insert("securityCode".into(), Value::String(cvv.clone()));
    }
    if let Some(exp) = &args.exp {
        let (month, year) = parse_exp(exp)?;
        obj.insert("expirationMonth".into(), json!(month));
        obj.insert("expirationYear".into(), json!(year));
    }

    // Optional amount/tip
    if let Some(tip) = args.tip_amount {
        obj.insert("tipAmount".into(), to_amount_number(tip));
    }

    // Optional customer/payment-method references
    if let Some(id) = &args.customer_id {
        obj.insert("customerId".into(), Value::String(id.clone()));
    }
    if let Some(id) = &args.payment_method_id {
        obj.insert("paymentMethodId".into(), Value::String(id.clone()));
    }

    // currencyId only if caller passed --currency-id (server defaults otherwise)
    if let Some(cid) = args.currency_id {
        obj.insert("currencyId".into(), json!(cid));
    }

    // Optional misc
    if let Some(rid) = &args.reference_id {
        obj.insert("referenceId".into(), Value::String(rid.clone()));
    }

    // L2 data — only when tax rate is provided
    if let Some(rate) = args.l2_tax_rate {
        let l2 = json!({ "salesTaxRate": to_amount_number(rate) });
        obj.insert("l2".into(), l2);
    }

    // L3 data — only when at least one l3 field is provided
    let has_l3 = args.l3_invoice.is_some() || args.l3_po.is_some() || !args.l3_product.is_empty();
    if has_l3 {
        let mut l3 = Map::new();
        if let Some(inv) = &args.l3_invoice {
            l3.insert("invoiceNumber".into(), Value::String(inv.clone()));
        }
        if let Some(po) = &args.l3_po {
            l3.insert("purchaseOrder".into(), Value::String(po.clone()));
        }
        if !args.l3_product.is_empty() {
            let products: Vec<Value> = args
                .l3_product
                .iter()
                .map(|s| parse_l3_product(s))
                .filter(|v| !v.is_null())
                .collect();
            if !products.is_empty() {
                l3.insert("products".into(), Value::Array(products));
            }
        }
        obj.insert("l3".into(), Value::Object(l3));
    }

    Ok(Value::Object(obj))
}

// ── Render helpers (pure — return String for golden-testability) ─────────────

/// Build the "quiet" output string: just the transaction ID (or `id`).
///
/// Returns `None` when neither field is present in the value.
pub(crate) fn transaction_quiet(v: &Value) -> Option<String> {
    v.get("transactionId")
        .or_else(|| v.get("id"))
        .and_then(|val| val.as_str())
        .map(|s| s.to_string())
}

/// Build the "table" output string with key transaction fields.
///
/// Fields shown: transactionId, status, amount (totalAmount), authCode, responseDescription.
pub(crate) fn transaction_table(v: &Value) -> String {
    let txn_id = v
        .get("transactionId")
        .or_else(|| v.get("id"))
        .and_then(|x| x.as_str())
        .unwrap_or("—");

    let status = v.get("status").and_then(|x| x.as_str()).unwrap_or("—");

    // amount field may be an object (AmountIsvDto), a JSON number, a string, or absent/null
    let amount = match v.get("amount") {
        Some(Value::Object(obj)) => obj
            .get("totalAmount")
            .and_then(|a| a.as_f64())
            .map(|f| format!("{f:.2}"))
            .unwrap_or_else(|| "—".to_string()),
        Some(Value::Number(n)) => n
            .as_f64()
            .map(|f| format!("{f:.2}"))
            .unwrap_or_else(|| "—".to_string()),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => "—".to_string(),
        Some(other) => other.to_string(),
    };

    let auth_code = v.get("authCode").and_then(|x| x.as_str()).unwrap_or("—");
    let response_desc = v
        .get("responseDescription")
        .and_then(|x| x.as_str())
        .unwrap_or("—");

    format!(
        "transactionId:      {txn_id}\nstatus:             {status}\namount:             {amount}\nauthCode:           {auth_code}\nresponseDescription: {response_desc}"
    )
}

// ── Task 1.8 — list helpers ──────────────────────────────────────────────────

/// Client-side filter for a slice of list-item `Value`s.
///
/// Returns a new `Vec<Value>` with cloned items that pass all active filters.
/// All filters are applied independently and AND-ed together (a transaction
/// must match every supplied filter to be included). Passing `None` for a
/// filter means "no constraint" for that field.
///
/// ## Status filter
/// Compares the item's `status` string field case-insensitively.
///
/// ## Date filter (`from` / `to`)
/// Compares the *date portion* of the item's `date` field (or
/// `transactionDateTime` as a fallback). Accepts ISO 8601 date-times or plain
/// YYYY-MM-DD strings; only the leading `YYYY-MM-DD` prefix is compared so
/// time-zone suffixes are ignored. Bounds are **inclusive**.
///
/// Items with a missing or unparseable `date` field are excluded when any
/// date filter is active.
///
/// **Important**: filters operate on the page returned by the server. They do
/// not trigger additional fetches or pagination.
pub(crate) fn filter_items(
    items: &[serde_json::Value],
    status: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) -> Vec<serde_json::Value> {
    items
        .iter()
        .filter(|item| {
            // Status filter (case-insensitive)
            if let Some(wanted) = status {
                let actual = item.get("status").and_then(|v| v.as_str()).unwrap_or("");
                if !actual.eq_ignore_ascii_case(wanted) {
                    return false;
                }
            }

            // Date filters — extract the YYYY-MM-DD prefix from the item's date field
            if from.is_some() || to.is_some() {
                let date_str = item
                    .get("date")
                    .or_else(|| item.get("transactionDateTime"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                // Grab just the date portion (first 10 chars = "YYYY-MM-DD")
                let item_date: &str = if date_str.len() >= 10 {
                    &date_str[..10]
                } else {
                    return false; // can't compare — exclude
                };

                if let Some(f) = from
                    && item_date < f
                {
                    return false;
                }
                if let Some(t) = to
                    && item_date > t
                {
                    return false;
                }
            }

            true
        })
        .cloned()
        .collect()
}

/// Build the "table" output string for the `list` command.
///
/// Columns (fixed widths via [`crate::cli::output::fit`]):
/// - `ID` (36)  `DATE` (10)  `STATUS` (12)  `TYPE` (14)  `AMOUNT` (10)  `CUSTOMER` (24)
///
/// Amount is read from `totalAmount` (top-level) or `amount.totalAmount` (nested object).
pub(crate) fn transaction_list_table(items: &[serde_json::Value]) -> String {
    use crate::cli::output::fit;

    let header = format!(
        "{:<36}  {:<10}  {:<12}  {:<14}  {:<10}  {:<24}",
        "ID", "DATE", "STATUS", "TYPE", "AMOUNT", "CUSTOMER"
    );
    let separator = "-".repeat(36 + 2 + 10 + 2 + 12 + 2 + 14 + 2 + 10 + 2 + 24);

    let mut rows = vec![header, separator];

    for item in items {
        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("—");

        let date = item
            .get("date")
            .and_then(|v| v.as_str())
            .map(|s| if s.len() >= 10 { &s[..10] } else { s })
            .unwrap_or("—");

        let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("—");

        let txn_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("—");

        // Amount: prefer top-level totalAmount, then nested amount.totalAmount
        let amount = match item.get("totalAmount").and_then(|v| v.as_f64()) {
            Some(f) => format!("{f:.2}"),
            None => match item.get("amount") {
                Some(serde_json::Value::Object(obj)) => obj
                    .get("totalAmount")
                    .and_then(|v| v.as_f64())
                    .map(|f| format!("{f:.2}"))
                    .unwrap_or_else(|| "—".to_string()),
                _ => "—".to_string(),
            },
        };

        let customer = item
            .get("customerName")
            .and_then(|v| v.as_str())
            .unwrap_or("—");

        rows.push(format!(
            "{}  {}  {}  {}  {}  {}",
            fit(id, 36),
            fit(date, 10),
            fit(status, 12),
            fit(txn_type, 14),
            fit(&amount, 10),
            fit(customer, 24),
        ));
    }

    rows.join("\n")
}

/// Build the "quiet" output string for the `list` command: one `id` per line.
pub(crate) fn transaction_list_quiet(items: &[serde_json::Value]) -> String {
    items
        .iter()
        .filter_map(|item| item.get("id").and_then(|v| v.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render the `list` command output according to the requested output format.
///
/// The `filtered` slice is the post-filter view; `total` is the server's
/// original total (preserved in the JSON envelope even though the item count
/// may be smaller after client-side filtering).
pub(crate) fn render_list(
    filtered: &[serde_json::Value],
    total: u64,
    fmt: crate::cli::output::OutputFormat,
    environment: &str,
) -> anyhow::Result<()> {
    use crate::cli::output::{Envelope, OutputFormat};
    match fmt {
        OutputFormat::Json => {
            let data = serde_json::json!({
                "items": filtered,
                "total": total,
                "filtered_count": filtered.len()
            });
            let envelope = Envelope::new("transaction_list", data, environment, None);
            println!("{}", serde_json::to_string_pretty(&envelope)?);
        }
        OutputFormat::Table => {
            println!("{}", transaction_list_table(filtered));
        }
        OutputFormat::Quiet => {
            let out = transaction_list_quiet(filtered);
            if !out.is_empty() {
                println!("{out}");
            }
        }
    }
    Ok(())
}

// ── Task 1.9 — inspect helper ────────────────────────────────────────────────

/// Build the rich "inspect" table view for a single `GetIsvTransactionResponseDto`.
///
/// Shows all key fields including the amount breakdown from the `amount` object
/// and the `availableOperations` array (used verbatim — no re-derivation).
///
/// Returns "—" for any field that is absent or null; never panics on missing data.
pub(crate) fn inspect_table(v: &serde_json::Value) -> String {
    let get_str = |key: &str| -> String {
        v.get(key)
            .and_then(|x| x.as_str())
            .unwrap_or("—")
            .to_string()
    };

    let txn_id = get_str("transactionId");
    let status = get_str("status");
    let currency = get_str("currency");
    let auth_code = get_str("authCode");
    let response_code = get_str("responseCode");
    let response_desc = get_str("responseDescription");
    let card_data_source = get_str("cardDataSource");
    let customer_pan = get_str("customerPan");
    let avs_response = get_str("avsResponse");

    // Amount breakdown from the nested AmountIsvDto object
    let (base_amount, surcharge_amount, tip_amount, total_amount) = match v.get("amount") {
        Some(serde_json::Value::Object(obj)) => {
            let fmt_f = |key: &str| -> String {
                obj.get(key)
                    .and_then(|v| v.as_f64())
                    .map(|f| format!("{f:.2}"))
                    .unwrap_or_else(|| "—".to_string())
            };
            (
                fmt_f("baseAmount"),
                fmt_f("surchargeAmount"),
                fmt_f("tipAmount"),
                fmt_f("totalAmount"),
            )
        }
        _ => (
            "—".to_string(),
            "—".to_string(),
            "—".to_string(),
            "—".to_string(),
        ),
    };

    // availableOperations — use the API's list verbatim
    let ops = match v.get("availableOperations") {
        Some(serde_json::Value::Array(arr)) => {
            if arr.is_empty() {
                "none".to_string()
            } else {
                arr.iter()
                    .filter_map(|op| op.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        }
        _ => "—".to_string(),
    };

    format!(
        "transactionId:       {txn_id}\n\
         status:              {status}\n\
         currency:            {currency}\n\
         authCode:            {auth_code}\n\
         responseCode:        {response_code}\n\
         responseDescription: {response_desc}\n\
         cardDataSource:      {card_data_source}\n\
         customerPan:         {customer_pan}\n\
         avsResponse:         {avs_response}\n\
         \n\
         Amount breakdown:\n\
           baseAmount:        {base_amount}\n\
           surchargeAmount:   {surcharge_amount}\n\
           tipAmount:         {tip_amount}\n\
           totalAmount:       {total_amount}\n\
         \n\
         Available operations: {ops}"
    )
}

// ── CLI handlers ─────────────────────────────────────────────────────────────

/// Selects which card-transaction endpoint to call.
pub(crate) enum CardTxnKind {
    Sale,
    Auth,
}

/// Render a transaction response according to the requested output format.
///
/// Writes to stdout. `environment` is embedded in the JSON envelope meta.
pub(crate) fn render_transaction(v: &Value, fmt: OutputFormat, environment: &str) -> Result<()> {
    match fmt {
        OutputFormat::Json => {
            let envelope = Envelope::new("transaction", v.clone(), environment, None);
            println!("{}", serde_json::to_string_pretty(&envelope)?);
        }
        OutputFormat::Quiet => {
            if let Some(id) = transaction_quiet(v) {
                println!("{id}");
            }
        }
        OutputFormat::Table => {
            println!("{}", transaction_table(v));
        }
    }
    Ok(())
}

/// Shared handler for card-transaction verbs that share the same request body
/// shape (`sale` and `auth`).
///
/// Both `sale` and `auth` build the body identically via `build_sale_body` and
/// differ only in which API endpoint they call.  All future card-verb handlers
/// that share this shape should call this function.
pub(crate) async fn execute_card_txn(
    profile: &str,
    output: OutputFormat,
    args: SaleArgs,
    kind: CardTxnKind,
) -> Result<()> {
    let body = build_sale_body(&args)?;
    let (p, api) = crate::build_client(profile)?;
    let result = match kind {
        CardTxnKind::Sale => api.sale(body).await?,
        CardTxnKind::Auth => api.auth_txn(body).await?,
    };
    render_transaction(&result, output, &p.name)
}

// ── Expiry parser (Task 1.2) ──────────────────────────────────────────────────

/// Parse a card expiry string in `MM/YY` or `MM/YYYY` format.
///
/// Returns `(month, 4-digit year)`.
///
/// # Rules
/// - Month must be 01–12; rejects 0 and > 12.
/// - 2-digit year is treated as 2000 + YY.
/// - 4-digit year is used as-is.
/// - Year tokens that are not exactly 2 or 4 digits are rejected.
/// - Non-numeric tokens and wrong separators are rejected.
pub(crate) fn parse_exp(s: &str) -> anyhow::Result<(u32, u32)> {
    let parts: Vec<&str> = s.splitn(2, '/').collect();
    if parts.len() != 2 {
        anyhow::bail!("expiry must be MM/YY or MM/YYYY (got '{s}')");
    }
    let month: u32 = parts[0]
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid month in expiry '{s}'"))?;
    if month == 0 || month > 12 {
        anyhow::bail!("month must be 01–12, got {month} (in '{s}')");
    }
    let year_token = parts[1];
    let year_len = year_token.len();
    if year_len != 2 && year_len != 4 {
        anyhow::bail!(
            "year must be exactly 2 digits (YY) or 4 digits (YYYY), got {year_len} digits (in '{s}')"
        );
    }
    let raw_year: u32 = year_token
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid year in expiry '{s}'"))?;
    let year = if year_len == 2 {
        2000 + raw_year
    } else {
        raw_year
    };
    Ok((month, year))
}

// ── Lifecycle body-builders (Tasks 1.6–1.7) ──────────────────────────────────

/// Build the JSON request body for a `capture` request.
///
/// `amount` is optional — omit for a full capture, pass for a partial capture.
///
/// # Wire fields
/// - `transactionId` (required)
/// - `amount` (optional, only when `Some`)
pub(crate) fn build_capture_body(
    transaction_id: &str,
    amount: Option<rust_decimal::Decimal>,
) -> Value {
    let mut obj = Map::new();
    obj.insert(
        "transactionId".into(),
        Value::String(transaction_id.to_string()),
    );
    if let Some(a) = amount {
        obj.insert("amount".into(), to_amount_number(a));
    }
    Value::Object(obj)
}

/// Build the JSON request body for a `void` request.
///
/// # Wire fields
/// - `transactionId` (required)
pub(crate) fn build_void_body(transaction_id: &str) -> Value {
    let mut obj = Map::new();
    obj.insert(
        "transactionId".into(),
        Value::String(transaction_id.to_string()),
    );
    Value::Object(obj)
}

/// Build the JSON request body for a `refund` (return) request.
///
/// `amount` is optional — omit for a full refund, pass for a partial refund.
/// `card_data_source` is required by the API (`ReturnRequestDto`); CLI default is `1` (Internet/ISV).
///
/// # Wire fields
/// - `transactionId` (required)
/// - `cardDataSource` (required, default 1)
/// - `amount` (optional, only when `Some`)
pub(crate) fn build_refund_body(
    transaction_id: &str,
    amount: Option<rust_decimal::Decimal>,
    card_data_source: i32,
) -> Value {
    let mut obj = Map::new();
    obj.insert(
        "transactionId".into(),
        Value::String(transaction_id.to_string()),
    );
    obj.insert("cardDataSource".into(), json!(card_data_source));
    if let Some(a) = amount {
        obj.insert("amount".into(), to_amount_number(a));
    }
    Value::Object(obj)
}

/// Build the JSON request body for a `settle` request.
///
/// **Note**: The API's `SettleRequestDto` accepts a `paymentProcessorId` (NOT a
/// `transactionId`). Settle closes/settles the open batch for the given payment
/// processor — it is a batch-level operation, not a per-transaction one.
///
/// # Wire fields
/// - `paymentProcessorId` (required)
pub(crate) fn build_settle_body(payment_processor_id: &str) -> Value {
    let mut obj = Map::new();
    obj.insert(
        "paymentProcessorId".into(),
        Value::String(payment_processor_id.to_string()),
    );
    Value::Object(obj)
}

/// Build the JSON request body for a `tip-adjustment` request.
///
/// # Wire fields
/// - `transactionId` (required)
/// - `tipAmount` (required, exact decimal via `to_amount_number`)
pub(crate) fn build_tip_adjust_body(
    transaction_id: &str,
    tip_amount: rust_decimal::Decimal,
) -> Value {
    let mut obj = Map::new();
    obj.insert(
        "transactionId".into(),
        Value::String(transaction_id.to_string()),
    );
    obj.insert("tipAmount".into(), to_amount_number(tip_amount));
    Value::Object(obj)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::str::FromStr;

    // ── parse_exp (Task 1.2) ──────────────────────────────────────────────────

    #[test]
    fn parses_mm_yy() {
        let (month, year) = parse_exp("12/26").unwrap();
        assert_eq!(month, 12);
        assert_eq!(year, 2026);
    }

    #[test]
    fn parses_mm_yyyy() {
        let (month, year) = parse_exp("03/2027").unwrap();
        assert_eq!(month, 3);
        assert_eq!(year, 2027);
    }

    #[test]
    fn rejects_bad_month() {
        assert!(parse_exp("13/26").is_err(), "month 13 must be rejected");
        assert!(parse_exp("00/26").is_err(), "month 0 must be rejected");
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_exp("abc").is_err(), "no slash must be rejected");
        assert!(parse_exp("1226").is_err(), "no slash must be rejected");
        assert!(parse_exp("ab/cd").is_err(), "non-numeric must be rejected");
    }

    #[test]
    fn rejects_malformed_year() {
        // 3-digit year must be rejected
        assert!(
            parse_exp("12/100").is_err(),
            "3-digit year must be rejected"
        );
        // 1-digit year must be rejected
        assert!(parse_exp("12/1").is_err(), "1-digit year must be rejected");
        // Sanity: valid 2- and 4-digit years still work
        assert!(parse_exp("12/26").is_ok(), "MM/YY must still be accepted");
        assert!(
            parse_exp("12/2026").is_ok(),
            "MM/YYYY must still be accepted"
        );
    }

    // ── build_sale_body (Task 1.3) ────────────────────────────────────────────

    /// Golden test: a representative SaleArgs maps to the exact expected JSON fields.
    ///
    /// Asserts:
    /// - `amount` is the JSON number `100.00` (not a string, not a float artifact)
    /// - `accountNumber` = "4111111111111111"
    /// - `expirationMonth` = 12 (integer)
    /// - `expirationYear` = 2026 (integer)
    /// - `securityCode` = "123"
    /// - `cardDataSource` = 1
    /// - `customerInitiatedTransaction` = false
    /// - `currencyId` is ABSENT (not passed → server defaults)
    /// - PAN appears in the body (not redacted at the body-building stage)
    #[test]
    fn build_sale_body_maps_flags_to_api_fields() {
        let args = SaleArgs {
            amount: Decimal::from_str("100.00").unwrap(),
            card: Some("4111111111111111".into()),
            exp: Some("12/26".into()),
            cvv: Some("123".into()),
            tip_amount: None,
            customer_id: None,
            payment_method_id: None,
            currency_id: None, // must be ABSENT from the output
            card_data_source: 1,
            l2_tax_rate: None,
            l3_invoice: None,
            l3_po: None,
            l3_product: vec![],
            reference_id: None,
        };

        let body = build_sale_body(&args).unwrap();

        // amount must be a JSON number exactly "100.00"
        assert!(
            body["amount"].is_number(),
            "amount must be a JSON number, got {:?}",
            body["amount"]
        );
        assert_eq!(
            serde_json::to_string(&body["amount"]).unwrap(),
            "100.00",
            "amount must serialise as 100.00 (exact decimal)"
        );

        // PAN
        assert_eq!(body["accountNumber"], "4111111111111111");

        // expiry
        assert_eq!(body["expirationMonth"], 12);
        assert_eq!(body["expirationYear"], 2026);

        // security code
        assert_eq!(body["securityCode"], "123");

        // always-present defaults
        assert_eq!(body["cardDataSource"], 1);
        assert_eq!(body["customerInitiatedTransaction"], false);

        // currencyId must be absent
        assert!(
            body.get("currencyId").is_none(),
            "currencyId must be absent when not passed"
        );

        // PAN appears in body (redaction is only for --debug logs)
        let serialized = serde_json::to_string(&body).unwrap();
        assert!(
            serialized.contains("4111111111111111"),
            "PAN must appear in the request body"
        );
    }

    #[test]
    fn build_sale_body_includes_currency_id_when_passed() {
        let args = SaleArgs {
            amount: Decimal::from_str("50.00").unwrap(),
            card: None,
            exp: None,
            cvv: None,
            tip_amount: None,
            customer_id: None,
            payment_method_id: None,
            currency_id: Some(840), // USD numeric code
            card_data_source: 1,
            l2_tax_rate: None,
            l3_invoice: None,
            l3_po: None,
            l3_product: vec![],
            reference_id: None,
        };
        let body = build_sale_body(&args).unwrap();
        assert_eq!(body["currencyId"], 840);
    }

    #[test]
    fn build_sale_body_includes_tip_amount_as_json_number() {
        let args = SaleArgs {
            amount: Decimal::from_str("100.00").unwrap(),
            card: None,
            exp: None,
            cvv: None,
            tip_amount: Some(Decimal::from_str("5.50").unwrap()),
            customer_id: None,
            payment_method_id: None,
            currency_id: None,
            card_data_source: 1,
            l2_tax_rate: None,
            l3_invoice: None,
            l3_po: None,
            l3_product: vec![],
            reference_id: None,
        };
        let body = build_sale_body(&args).unwrap();
        assert!(body["tipAmount"].is_number());
        assert_eq!(serde_json::to_string(&body["tipAmount"]).unwrap(), "5.50");
    }

    #[test]
    fn build_sale_body_l2_sets_sales_tax_rate() {
        let args = SaleArgs {
            amount: Decimal::from_str("100.00").unwrap(),
            card: None,
            exp: None,
            cvv: None,
            tip_amount: None,
            customer_id: None,
            payment_method_id: None,
            currency_id: None,
            card_data_source: 1,
            l2_tax_rate: Some(Decimal::from_str("0.08").unwrap()),
            l3_invoice: None,
            l3_po: None,
            l3_product: vec![],
            reference_id: None,
        };
        let body = build_sale_body(&args).unwrap();
        assert!(body.get("l2").is_some(), "l2 must be present");
        assert!(body["l2"]["salesTaxRate"].is_number());
        assert_eq!(
            serde_json::to_string(&body["l2"]["salesTaxRate"]).unwrap(),
            "0.08"
        );
    }

    #[test]
    fn build_sale_body_l3_sets_invoice_and_products() {
        let args = SaleArgs {
            amount: Decimal::from_str("200.00").unwrap(),
            card: None,
            exp: None,
            cvv: None,
            tip_amount: None,
            customer_id: None,
            payment_method_id: None,
            currency_id: None,
            card_data_source: 1,
            l2_tax_rate: None,
            l3_invoice: Some("INV-001".into()),
            l3_po: Some("PO-002".into()),
            l3_product: vec!["Widget,SKU-1,10.00,EA,2".into()],
            reference_id: None,
        };
        let body = build_sale_body(&args).unwrap();
        assert!(body.get("l3").is_some(), "l3 must be present");
        assert_eq!(body["l3"]["invoiceNumber"], "INV-001");
        assert_eq!(body["l3"]["purchaseOrder"], "PO-002");
        let products = body["l3"]["products"].as_array().unwrap();
        assert_eq!(products.len(), 1);
        assert_eq!(products[0]["description"], "Widget");
        assert_eq!(products[0]["productCode"], "SKU-1");
        assert!(products[0]["unitPrice"].is_number());
        assert_eq!(
            serde_json::to_string(&products[0]["unitPrice"]).unwrap(),
            "10.00"
        );
        assert_eq!(products[0]["unitOfMeasure"], "EA");
    }

    // ── render helpers (Task 1.5) ─────────────────────────────────────────────

    fn sample_txn_response() -> Value {
        json!({
            "transactionId": "t_123abc",
            "status": "Approved",
            "amount": {
                "totalAmount": 100.00,
                "baseAmount": 100.00,
                "surchargeAmount": 0.0,
                "tipAmount": 0.0
            },
            "authCode": "AUTH99",
            "responseDescription": "Approved"
        })
    }

    #[test]
    fn transaction_quiet_returns_transaction_id() {
        let v = sample_txn_response();
        assert_eq!(transaction_quiet(&v), Some("t_123abc".to_string()));
    }

    #[test]
    fn transaction_quiet_falls_back_to_id_field() {
        let v = json!({ "id": "fallback-id", "status": "Approved" });
        assert_eq!(transaction_quiet(&v), Some("fallback-id".to_string()));
    }

    #[test]
    fn transaction_quiet_returns_none_when_no_id() {
        let v = json!({ "status": "Approved" });
        assert_eq!(transaction_quiet(&v), None);
    }

    #[test]
    fn transaction_table_contains_key_fields() {
        let v = sample_txn_response();
        let table = transaction_table(&v);
        assert!(table.contains("t_123abc"), "must contain transactionId");
        assert!(table.contains("Approved"), "must contain status");
        assert!(table.contains("AUTH99"), "must contain authCode");
    }

    #[test]
    fn transaction_table_reads_total_amount_from_amount_object() {
        let v = sample_txn_response();
        let table = transaction_table(&v);
        // totalAmount = 100.0 → should appear as 100.00
        assert!(
            table.contains("100.00"),
            "must contain formatted totalAmount"
        );
    }

    #[test]
    fn transaction_table_amount_null_renders_dash() {
        let v = json!({
            "transactionId": "t_null",
            "status": "Approved",
            "amount": null,
            "authCode": "A1",
            "responseDescription": "OK"
        });
        let table = transaction_table(&v);
        assert!(table.contains("—"), "null amount must render as —");
    }

    #[test]
    fn transaction_table_amount_missing_renders_dash() {
        let v = json!({
            "transactionId": "t_missing",
            "status": "Approved",
            "authCode": "A2",
            "responseDescription": "OK"
        });
        let table = transaction_table(&v);
        assert!(table.contains("—"), "missing amount must render as —");
    }

    #[test]
    fn transaction_table_amount_string_renders_unquoted() {
        let v = json!({
            "transactionId": "t_str",
            "status": "Approved",
            "amount": "99.99",
            "authCode": "A3",
            "responseDescription": "OK"
        });
        let table = transaction_table(&v);
        // Must contain "99.99" without surrounding quotes
        assert!(
            table.contains("99.99"),
            "string amount must render unquoted"
        );
        assert!(
            !table.contains("\"99.99\""),
            "string amount must NOT be quoted"
        );
    }

    #[test]
    fn render_transaction_json_envelope_shape() {
        let v = sample_txn_response();
        // Capture stdout by rendering to a string via the envelope directly
        let envelope = Envelope::new("transaction", v.clone(), "sandbox", None);
        let json_str = serde_json::to_string_pretty(&envelope).unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["object"], "transaction");
        assert_eq!(parsed["data"]["transactionId"], "t_123abc");
        assert_eq!(parsed["meta"]["environment"], "sandbox");
    }

    #[test]
    fn parse_l3_product_parses_all_fields() {
        let v = parse_l3_product("Widget A,SKU-42,9.99,EA,5");
        assert_eq!(v["description"], "Widget A");
        assert_eq!(v["productCode"], "SKU-42");
        assert!(v["unitPrice"].is_number());
        assert_eq!(serde_json::to_string(&v["unitPrice"]).unwrap(), "9.99");
        assert_eq!(v["unitOfMeasure"], "EA");
        assert!(v["quantity"].is_number());
    }

    #[test]
    fn parse_l3_product_handles_partial_fields() {
        // Only description provided
        let v = parse_l3_product("Widget Only");
        assert_eq!(v["description"], "Widget Only");
        assert!(v.get("productCode").is_none());
    }

    #[test]
    fn parse_l3_product_empty_string_returns_null() {
        assert!(
            parse_l3_product("").is_null(),
            "empty string must return Null"
        );
        assert!(
            parse_l3_product("   ").is_null(),
            "whitespace-only must return Null"
        );
    }

    #[test]
    fn build_sale_body_empty_l3_product_produces_no_products_entry() {
        let args = SaleArgs {
            amount: Decimal::from_str("50.00").unwrap(),
            card: None,
            exp: None,
            cvv: None,
            tip_amount: None,
            customer_id: None,
            payment_method_id: None,
            currency_id: None,
            card_data_source: 1,
            l2_tax_rate: None,
            l3_invoice: Some("INV-EMPTY".into()),
            l3_po: None,
            l3_product: vec!["".into(), "   ".into()],
            reference_id: None,
        };
        let body = build_sale_body(&args).unwrap();
        // l3 is present (because l3_invoice was set), but products must be absent
        assert!(body.get("l3").is_some(), "l3 must be present");
        assert!(
            body["l3"].get("products").is_none(),
            "products must be absent when all --l3-product values are blank"
        );
    }

    // ── build_capture_body (Task 1.6) ─────────────────────────────────────────

    #[test]
    fn build_capture_body_full_capture_has_no_amount() {
        let body = build_capture_body("txn-uuid-001", None);
        assert_eq!(body["transactionId"], "txn-uuid-001");
        assert!(
            body.get("amount").is_none(),
            "amount must be absent for full capture"
        );
    }

    #[test]
    fn build_capture_body_partial_capture_has_exact_amount() {
        let body = build_capture_body("txn-uuid-002", Some(Decimal::from_str("50.00").unwrap()));
        assert_eq!(body["transactionId"], "txn-uuid-002");
        assert!(body["amount"].is_number(), "amount must be a JSON number");
        assert_eq!(
            serde_json::to_string(&body["amount"]).unwrap(),
            "50.00",
            "partial capture amount must serialise as exact decimal"
        );
    }

    // ── build_void_body (Task 1.6) ────────────────────────────────────────────

    #[test]
    fn build_void_body_sets_transaction_id() {
        let body = build_void_body("txn-void-123");
        assert_eq!(body["transactionId"], "txn-void-123");
        // No extra fields expected
        let obj = body.as_object().unwrap();
        assert_eq!(obj.len(), 1, "void body must have exactly one field");
    }

    // ── build_refund_body (Task 1.6) ──────────────────────────────────────────

    #[test]
    fn build_refund_body_full_refund_defaults_card_data_source() {
        let body = build_refund_body("txn-refund-001", None, 1);
        assert_eq!(body["transactionId"], "txn-refund-001");
        assert_eq!(body["cardDataSource"], 1);
        assert!(
            body.get("amount").is_none(),
            "amount must be absent for full refund"
        );
    }

    #[test]
    fn build_refund_body_partial_refund_has_exact_amount() {
        let body = build_refund_body(
            "txn-refund-002",
            Some(Decimal::from_str("25.00").unwrap()),
            1,
        );
        assert_eq!(body["transactionId"], "txn-refund-002");
        assert_eq!(body["cardDataSource"], 1);
        assert!(body["amount"].is_number());
        assert_eq!(serde_json::to_string(&body["amount"]).unwrap(), "25.00");
    }

    #[test]
    fn build_refund_body_respects_custom_card_data_source() {
        let body = build_refund_body("txn-refund-003", None, 7);
        assert_eq!(body["cardDataSource"], 7);
    }

    // ── build_settle_body (Task 1.6) ──────────────────────────────────────────

    #[test]
    fn build_settle_body_sets_payment_processor_id() {
        let body = build_settle_body("proc-uuid-abc");
        assert_eq!(body["paymentProcessorId"], "proc-uuid-abc");
        // Must NOT have transactionId — settle is a batch-level op
        assert!(
            body.get("transactionId").is_none(),
            "settle body must NOT have transactionId"
        );
        let obj = body.as_object().unwrap();
        assert_eq!(
            obj.len(),
            1,
            "settle body must have exactly one field: paymentProcessorId"
        );
    }

    // ── build_tip_adjust_body (Task 1.6) ──────────────────────────────────────

    #[test]
    fn build_tip_adjust_body_sets_transaction_id_and_tip_amount() {
        let body = build_tip_adjust_body("txn-tip-999", Decimal::from_str("3.50").unwrap());
        assert_eq!(body["transactionId"], "txn-tip-999");
        assert!(
            body["tipAmount"].is_number(),
            "tipAmount must be a JSON number"
        );
        assert_eq!(
            serde_json::to_string(&body["tipAmount"]).unwrap(),
            "3.50",
            "tipAmount must serialise as exact decimal"
        );
    }

    // ── filter_items (Task 1.8) ───────────────────────────────────────────────

    fn sample_list_items() -> Vec<Value> {
        vec![
            json!({
                "id": "txn-001",
                "status": "Approved",
                "date": "2026-05-01T10:00:00Z",
                "totalAmount": 100.00,
                "customerName": "Alice",
                "type": "Sale"
            }),
            json!({
                "id": "txn-002",
                "status": "Declined",
                "date": "2026-05-15T12:00:00Z",
                "totalAmount": 50.00,
                "customerName": "Bob",
                "type": "Sale"
            }),
            json!({
                "id": "txn-003",
                "status": "Approved",
                "date": "2026-06-01T09:00:00Z",
                "totalAmount": 200.00,
                "customerName": "Carol",
                "type": "Refund"
            }),
        ]
    }

    #[test]
    fn filter_items_no_filters_returns_all() {
        let items = sample_list_items();
        let result = filter_items(&items, None, None, None);
        assert_eq!(result.len(), 3, "no filters should return all 3 items");
    }

    #[test]
    fn filter_items_status_match_case_insensitive() {
        let items = sample_list_items();

        let approved = filter_items(&items, Some("approved"), None, None);
        assert_eq!(approved.len(), 2, "should match 2 Approved transactions");
        assert!(approved.iter().all(|i| i["status"] == "Approved"));

        let declined = filter_items(&items, Some("DECLINED"), None, None);
        assert_eq!(declined.len(), 1);
        assert_eq!(declined[0]["id"], "txn-002");
    }

    #[test]
    fn filter_items_status_no_match_returns_empty() {
        let items = sample_list_items();
        let result = filter_items(&items, Some("Voided"), None, None);
        assert_eq!(result.len(), 0, "no Voided items should return empty");
    }

    #[test]
    fn filter_items_from_date_inclusive_lower_bound() {
        let items = sample_list_items();
        // from = 2026-05-15 → should include txn-002 and txn-003, exclude txn-001
        let result = filter_items(&items, None, Some("2026-05-15"), None);
        assert_eq!(result.len(), 2);
        let ids: Vec<&str> = result.iter().map(|i| i["id"].as_str().unwrap()).collect();
        assert!(
            ids.contains(&"txn-002"),
            "txn-002 (on boundary) must be included"
        );
        assert!(ids.contains(&"txn-003"), "txn-003 (after) must be included");
        assert!(
            !ids.contains(&"txn-001"),
            "txn-001 (before) must be excluded"
        );
    }

    #[test]
    fn filter_items_to_date_inclusive_upper_bound() {
        let items = sample_list_items();
        // to = 2026-05-15 → should include txn-001 and txn-002, exclude txn-003
        let result = filter_items(&items, None, None, Some("2026-05-15"));
        assert_eq!(result.len(), 2);
        let ids: Vec<&str> = result.iter().map(|i| i["id"].as_str().unwrap()).collect();
        assert!(
            ids.contains(&"txn-001"),
            "txn-001 (before) must be included"
        );
        assert!(
            ids.contains(&"txn-002"),
            "txn-002 (on boundary) must be included"
        );
        assert!(
            !ids.contains(&"txn-003"),
            "txn-003 (after) must be excluded"
        );
    }

    #[test]
    fn filter_items_date_range_combination() {
        let items = sample_list_items();
        // from = 2026-05-01, to = 2026-05-31 → only txn-001 and txn-002
        let result = filter_items(&items, None, Some("2026-05-01"), Some("2026-05-31"));
        assert_eq!(result.len(), 2);
        let ids: Vec<&str> = result.iter().map(|i| i["id"].as_str().unwrap()).collect();
        assert!(ids.contains(&"txn-001"));
        assert!(ids.contains(&"txn-002"));
        assert!(!ids.contains(&"txn-003"));
    }

    #[test]
    fn filter_items_status_and_date_combined() {
        let items = sample_list_items();
        // status=Approved AND from=2026-06-01 → only txn-003
        let result = filter_items(&items, Some("Approved"), Some("2026-06-01"), None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["id"], "txn-003");
    }

    #[test]
    fn filter_items_missing_date_excluded_when_date_filter_active() {
        let items = vec![json!({ "id": "no-date", "status": "Approved" })];
        let result = filter_items(&items, None, Some("2026-01-01"), None);
        assert_eq!(
            result.len(),
            0,
            "item with no date should be excluded when date filter active"
        );
    }

    // ── transaction_list_table golden test (Task 1.8) ─────────────────────────

    #[test]
    fn transaction_list_table_contains_header_and_row() {
        let items = sample_list_items();
        let table = transaction_list_table(&items);

        // Header
        assert!(table.contains("ID"), "must contain ID header");
        assert!(table.contains("DATE"), "must contain DATE header");
        assert!(table.contains("STATUS"), "must contain STATUS header");
        assert!(table.contains("TYPE"), "must contain TYPE header");
        assert!(table.contains("AMOUNT"), "must contain AMOUNT header");
        assert!(table.contains("CUSTOMER"), "must contain CUSTOMER header");

        // Row data
        assert!(table.contains("txn-001"), "must contain first txn id");
        assert!(
            table.contains("2026-05-01"),
            "must contain date portion only"
        );
        assert!(table.contains("Approved"), "must contain status");
        assert!(table.contains("100.00"), "must contain formatted amount");
        assert!(table.contains("Alice"), "must contain customer name");
    }

    #[test]
    fn transaction_list_table_empty_items_shows_header_only() {
        let table = transaction_list_table(&[]);
        assert!(
            table.contains("ID"),
            "header must still appear for empty list"
        );
        // No data rows beyond header + separator
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 2, "empty list: only header + separator");
    }

    #[test]
    fn transaction_list_table_reads_nested_amount_object() {
        let items = vec![json!({
            "id": "txn-nested",
            "status": "Approved",
            "date": "2026-06-02T00:00:00Z",
            "amount": { "totalAmount": 75.50 },
            "customerName": "Dave",
            "type": "Sale"
        })];
        let table = transaction_list_table(&items);
        assert!(
            table.contains("75.50"),
            "must read totalAmount from nested amount object"
        );
    }

    // ── transaction_list_quiet golden test (Task 1.8) ─────────────────────────

    #[test]
    fn transaction_list_quiet_one_id_per_line() {
        let items = sample_list_items();
        let quiet = transaction_list_quiet(&items);
        let lines: Vec<&str> = quiet.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "txn-001");
        assert_eq!(lines[1], "txn-002");
        assert_eq!(lines[2], "txn-003");
    }

    #[test]
    fn transaction_list_quiet_empty_returns_empty_string() {
        let quiet = transaction_list_quiet(&[]);
        assert_eq!(quiet, "", "empty items must produce empty string");
    }

    // ── inspect_table golden test (Task 1.9) ─────────────────────────────────

    fn sample_inspect_response() -> Value {
        json!({
            "transactionId": "txn-inspect-abc",
            "status": "Approved",
            "currency": "USD",
            "authCode": "AUTH42",
            "responseCode": "00",
            "responseDescription": "Approved",
            "cardDataSource": "Internet",
            "customerPan": "411111XXXXXX1111",
            "avsResponse": "Y",
            "amount": {
                "baseAmount": 95.00,
                "surchargeAmount": 2.50,
                "tipAmount": 5.00,
                "totalAmount": 102.50
            },
            "availableOperations": ["void", "refund", "tip-adjust"]
        })
    }

    #[test]
    fn inspect_table_contains_key_fields() {
        let v = sample_inspect_response();
        let table = inspect_table(&v);

        assert!(
            table.contains("txn-inspect-abc"),
            "must contain transactionId"
        );
        assert!(table.contains("Approved"), "must contain status");
        assert!(table.contains("USD"), "must contain currency");
        assert!(table.contains("AUTH42"), "must contain authCode");
        assert!(table.contains("00"), "must contain responseCode");
        assert!(table.contains("Internet"), "must contain cardDataSource");
        assert!(
            table.contains("411111XXXXXX1111"),
            "must contain customerPan"
        );
        assert!(table.contains("Y"), "must contain avsResponse");
    }

    #[test]
    fn inspect_table_shows_amount_breakdown() {
        let v = sample_inspect_response();
        let table = inspect_table(&v);

        assert!(table.contains("95.00"), "must contain baseAmount");
        assert!(table.contains("2.50"), "must contain surchargeAmount");
        assert!(table.contains("5.00"), "must contain tipAmount");
        assert!(table.contains("102.50"), "must contain totalAmount");
    }

    #[test]
    fn inspect_table_shows_available_operations() {
        let v = sample_inspect_response();
        let table = inspect_table(&v);

        assert!(table.contains("void"), "must contain void operation");
        assert!(table.contains("refund"), "must contain refund operation");
        assert!(
            table.contains("tip-adjust"),
            "must contain tip-adjust operation"
        );
        assert!(
            table.contains("Available operations:"),
            "must contain Available operations label"
        );
    }

    #[test]
    fn inspect_table_empty_available_operations_shows_none() {
        let v = json!({
            "transactionId": "txn-no-ops",
            "status": "Settled",
            "availableOperations": []
        });
        let table = inspect_table(&v);
        assert!(
            table.contains("none"),
            "empty availableOperations must render as 'none'"
        );
    }

    #[test]
    fn inspect_table_missing_fields_render_dash() {
        // Completely minimal response — must not panic
        let v = json!({ "transactionId": "txn-minimal" });
        let table = inspect_table(&v);
        assert!(table.contains("txn-minimal"), "must contain transactionId");
        // All missing fields should render as "—"
        let dash_count = table.matches('—').count();
        assert!(
            dash_count >= 4,
            "missing fields must render as — (got {dash_count} dashes)"
        );
    }

    #[test]
    fn inspect_table_null_available_operations_renders_dash() {
        let v = json!({
            "transactionId": "txn-null-ops",
            "availableOperations": null
        });
        let table = inspect_table(&v);
        // Should contain "—" for the ops field (null, not an array)
        assert!(
            table.contains('—'),
            "null availableOperations must render as —"
        );
    }
}
