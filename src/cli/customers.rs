//! CLI body builders and render helpers for the Customers / Vault command group
//! (`flute customers …`).
//!
//! All body builders and render helpers are **pure functions** — no I/O, no
//! network — so they are trivially unit-testable with golden assertions.

use anyhow::Result;
use serde_json::{Map, Value, json};

use crate::cli::ach::{AccountHolderTypeArg, AccountTypeArg};
use crate::cli::output::{Envelope, OutputFormat, fit};
use crate::cli::transactions::parse_exp;

// ── Body-builders ─────────────────────────────────────────────────────────────

/// Build the JSON request body for `customers create` and `customers update`.
///
/// All fields are optional (the spec has no required fields on Create/Update).
/// Fields that are `None` are **omitted** from the body so the API receives
/// only what the user explicitly provided.
///
/// # Field mapping
/// | Arg            | Wire key              |
/// |----------------|-----------------------|
/// | `first_name`   | `firstName`           |
/// | `last_name`    | `lastName`            |
/// | `company`      | `companyName`         |
/// | `email`        | `email`               |
/// | `mobile`       | `mobilePhoneNumber`   |
pub fn build_customer_body(
    first_name: Option<&str>,
    last_name: Option<&str>,
    company: Option<&str>,
    email: Option<&str>,
    mobile: Option<&str>,
) -> Value {
    let mut obj = Map::new();
    if let Some(v) = first_name {
        obj.insert("firstName".into(), Value::String(v.to_string()));
    }
    if let Some(v) = last_name {
        obj.insert("lastName".into(), Value::String(v.to_string()));
    }
    if let Some(v) = company {
        obj.insert("companyName".into(), Value::String(v.to_string()));
    }
    if let Some(v) = email {
        obj.insert("email".into(), Value::String(v.to_string()));
    }
    if let Some(v) = mobile {
        obj.insert("mobilePhoneNumber".into(), Value::String(v.to_string()));
    }
    Value::Object(obj)
}

/// Build the JSON request body for `customers add-card`.
///
/// `name` is optional; `pan`, `exp`, and `cvv` are required by the CLI flags.
/// `exp` is parsed via [`parse_exp`] (`"MM/YY"` or `"MM/YYYY"`).
///
/// # Field mapping
/// | Arg   | Wire key            |
/// |-------|---------------------|
/// | `name`| `name` (omitted if None) |
/// | `pan` | `pan`               |
/// | `exp` | `expirationMonth` + `expirationYear` |
/// | `cvv` | `securityCode`      |
pub fn build_add_card_body(
    name: Option<&str>,
    pan: &str,
    exp: &str,
    cvv: Option<&str>,
) -> Result<Value> {
    let (month, year) = parse_exp(exp)?;
    let mut obj = Map::new();
    if let Some(n) = name {
        obj.insert("name".into(), Value::String(n.to_string()));
    }
    obj.insert("pan".into(), Value::String(pan.to_string()));
    obj.insert("expirationMonth".into(), json!(month));
    obj.insert("expirationYear".into(), json!(year));
    if let Some(c) = cvv {
        obj.insert("securityCode".into(), Value::String(c.to_string()));
    }
    Ok(Value::Object(obj))
}

/// Build the JSON request body for `customers add-ach`.
///
/// `name`, `account_holder_type`, and `tax_id` are optional and omitted when
/// `None`. `account_type` defaults to `Checking` (1) on the CLI default.
///
/// # Field mapping
/// | Arg                   | Wire key              |
/// |-----------------------|-----------------------|
/// | `name`                | `name` (omitted if None) |
/// | `account`             | `accountNumber`       |
/// | `routing`             | `routingNumber`       |
/// | `account_type`        | `accountType` (int)   |
/// | `account_holder_type` | `accountHolderType` (int, omitted if None) |
/// | `tax_id`              | `taxId` (omitted if None) |
pub fn build_add_ach_body(
    name: Option<&str>,
    account: &str,
    routing: &str,
    account_type: AccountTypeArg,
    account_holder_type: Option<AccountHolderTypeArg>,
    tax_id: Option<&str>,
) -> Value {
    let mut obj = Map::new();
    if let Some(n) = name {
        obj.insert("name".into(), Value::String(n.to_string()));
    }
    obj.insert("accountNumber".into(), Value::String(account.to_string()));
    obj.insert("routingNumber".into(), Value::String(routing.to_string()));
    obj.insert("accountType".into(), json!(account_type.to_api_int()));
    if let Some(ht) = account_holder_type {
        obj.insert("accountHolderType".into(), json!(ht.to_api_int()));
    }
    if let Some(tid) = tax_id {
        obj.insert("taxId".into(), Value::String(tid.to_string()));
    }
    Value::Object(obj)
}

// ── Render helpers ────────────────────────────────────────────────────────────

/// Render a single customer response (`GetCustomerResponseIsvDto`).
///
/// - `json`  → `Envelope { object: "customer", data: v, … }`
/// - `table` → key-value list of core fields
/// - `quiet` → just the customer `id`
pub fn render_customer(v: &Value, fmt: OutputFormat, environment: &str) -> anyhow::Result<()> {
    match fmt {
        OutputFormat::Json => {
            let env = Envelope::new("customer", v.clone(), environment, None);
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        OutputFormat::Table => {
            println!("{}", customer_table(v));
        }
        OutputFormat::Quiet => {
            if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
                println!("{id}");
            }
        }
    }
    Ok(())
}

/// Build the table string for a single customer (pure helper, golden-testable).
pub(crate) fn customer_table(v: &Value) -> String {
    let get = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("—");
    format!(
        "id:                 {}\nfirstName:          {}\nlastName:           {}\nemail:              {}\nmobilePhoneNumber:  {}\ncompanyName:        {}\ncreatedOn:          {}",
        get("id"),
        get("firstName"),
        get("lastName"),
        get("email"),
        get("mobilePhoneNumber"),
        get("companyName"),
        get("createdOn"),
    )
}

/// Render a customer list response (`GetCustomerPageResponseIsvDto`).
///
/// The server wraps items in `{items, total}`.  This function reads defensively:
/// - First tries `v["items"]` as an array (standard page wrapper).
/// - Falls back to treating `v` itself as an array.
///
/// - `json`  → `Envelope { object: "customer_list", data: {items, total}, … }`
/// - `table` → columnar table via [`customer_list_table`]
/// - `quiet` → one ID per line
pub fn render_customer_list(v: &Value, fmt: OutputFormat, environment: &str) -> anyhow::Result<()> {
    let (items, total) = extract_customer_items(v);

    match fmt {
        OutputFormat::Json => {
            let data = json!({ "items": items, "total": total });
            let env = Envelope::new("customer_list", data, environment, None);
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        OutputFormat::Table => {
            println!("{}", customer_list_table(&items));
        }
        OutputFormat::Quiet => {
            for item in &items {
                if let Some(id) = item.get("id").and_then(|x| x.as_str()) {
                    println!("{id}");
                }
            }
        }
    }
    Ok(())
}

/// Extract the items array and total from a page response (defensive).
fn extract_customer_items(v: &Value) -> (Vec<Value>, u64) {
    if let Some(items_val) = v.get("items") {
        let items = items_val.as_array().cloned().unwrap_or_default();
        let total = v.get("total").and_then(|t| t.as_u64()).unwrap_or(0);
        (items, total)
    } else if let Some(arr) = v.as_array() {
        let total = arr.len() as u64;
        (arr.clone(), total)
    } else {
        (Vec::new(), 0)
    }
}

/// Build the table string for a customer list (pure helper, golden-testable).
///
/// Columns: ID (36), NAME (28), EMAIL (28), PHONE (14), CREATED (10)
pub(crate) fn customer_list_table(items: &[Value]) -> String {
    let header = format!(
        "{:<36}  {:<28}  {:<28}  {:<14}  {:<10}",
        "ID", "NAME", "EMAIL", "PHONE", "CREATED"
    );
    let separator = "-".repeat(36 + 2 + 28 + 2 + 28 + 2 + 14 + 2 + 10);
    let mut rows = vec![header, separator];

    for item in items {
        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("—");
        let first = item.get("firstName").and_then(|v| v.as_str()).unwrap_or("");
        let last = item.get("lastName").and_then(|v| v.as_str()).unwrap_or("");
        let name = if first.is_empty() && last.is_empty() {
            "—".to_string()
        } else {
            format!("{first} {last}").trim().to_string()
        };
        let email = item.get("email").and_then(|v| v.as_str()).unwrap_or("—");
        let phone = item
            .get("mobilePhoneNumber")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let created = item
            .get("createdOn")
            .and_then(|v| v.as_str())
            .map(|s| if s.len() >= 10 { &s[..10] } else { s })
            .unwrap_or("—");

        rows.push(format!(
            "{}  {}  {}  {}  {}",
            fit(id, 36),
            fit(&name, 28),
            fit(email, 28),
            fit(phone, 14),
            fit(created, 10),
        ));
    }

    rows.join("\n")
}

/// Build the table string for a single payment method (pure helper, golden-testable).
pub(crate) fn payment_method_table(v: &Value) -> String {
    let get_str = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("—");
    let pan_or_acct = v
        .get("panMask")
        .and_then(|x| x.as_str())
        .or_else(|| v.get("accountNumber").and_then(|x| x.as_str()))
        .unwrap_or("—");
    let exp = match (
        v.get("expirationMonth").and_then(|x| x.as_u64()),
        v.get("expirationYear").and_then(|x| x.as_u64()),
    ) {
        (Some(m), Some(y)) => format!("{m:02}/{y}"),
        _ => "—".to_string(),
    };
    let is_default = v
        .get("isDefault")
        .and_then(|x| x.as_bool())
        .map(|b| if b { "yes" } else { "no" })
        .unwrap_or("—");
    format!(
        "id:               {}\ntypeName:         {}\npan/account:      {}\nexp:              {}\nisDefault:        {}",
        get_str("id"),
        get_str("typeName"),
        pan_or_acct,
        exp,
        is_default,
    )
}

/// Extract just the payment-method `id` for quiet output (pure helper).
pub(crate) fn payment_method_quiet(v: &Value) -> Option<&str> {
    v.get("id").and_then(|x| x.as_str())
}

/// Render a single payment-method object returned by `add-card` or `add-ach`.
///
/// - `json`  → `Envelope { object: "payment_method", data: v, … }`
/// - `table` → key-value lines for id, typeName, panMask/accountNumber, exp, isDefault
/// - `quiet` → just the `id` as a bare string
pub fn render_payment_method(v: &Value, fmt: OutputFormat, env: &str) -> anyhow::Result<()> {
    match fmt {
        OutputFormat::Json => {
            let envelope = Envelope::new("payment_method", v.clone(), env, None);
            println!("{}", serde_json::to_string_pretty(&envelope)?);
        }
        OutputFormat::Table => {
            println!("{}", payment_method_table(v));
        }
        OutputFormat::Quiet => {
            if let Some(id) = payment_method_quiet(v) {
                println!("{id}");
            }
        }
    }
    Ok(())
}

/// Render a list of payment methods (`GetCustomerPaymentMethodsResponseIsvDto`).
///
/// Response may be an array or `{items, …}` — read defensively.
///
/// - `json`  → `Envelope { object: "payment_methods", … }`
/// - `table` → columnar table via [`payment_methods_table`]
/// - `quiet` → one ID per line
pub fn render_payment_methods(
    v: &Value,
    fmt: OutputFormat,
    environment: &str,
) -> anyhow::Result<()> {
    let items = if let Some(arr) = v.as_array() {
        arr.clone()
    } else if let Some(items_val) = v.get("items").and_then(|x| x.as_array()) {
        items_val.clone()
    } else {
        Vec::new()
    };

    match fmt {
        OutputFormat::Json => {
            let env = Envelope::new("payment_methods", v.clone(), environment, None);
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        OutputFormat::Table => {
            println!("{}", payment_methods_table(&items));
        }
        OutputFormat::Quiet => {
            for item in &items {
                if let Some(id) = item.get("id").and_then(|x| x.as_str()) {
                    println!("{id}");
                }
            }
        }
    }
    Ok(())
}

/// Build the table string for a payment-methods list (pure helper, golden-testable).
///
/// Columns: ID (36), TYPE (12), PAN/ACCT (20), EXP (7), DEFAULT (7)
pub(crate) fn payment_methods_table(items: &[Value]) -> String {
    let header = format!(
        "{:<36}  {:<12}  {:<20}  {:<7}  {:<7}",
        "ID", "TYPE", "PAN/ACCT", "EXP", "DEFAULT"
    );
    let separator = "-".repeat(36 + 2 + 12 + 2 + 20 + 2 + 7 + 2 + 7);
    let mut rows = vec![header, separator];

    for item in items {
        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("—");
        let type_name = item.get("typeName").and_then(|v| v.as_str()).unwrap_or("—");
        // Pan mask for cards; accountNumber for ACH
        let pan_or_acct = item
            .get("panMask")
            .and_then(|v| v.as_str())
            .or_else(|| item.get("accountNumber").and_then(|v| v.as_str()))
            .unwrap_or("—");
        // Expiration: month/year for cards
        let exp = match (
            item.get("expirationMonth").and_then(|v| v.as_u64()),
            item.get("expirationYear").and_then(|v| v.as_u64()),
        ) {
            (Some(m), Some(y)) => format!("{m:02}/{y}"),
            _ => "—".to_string(),
        };
        let is_default = item
            .get("isDefault")
            .and_then(|v| v.as_bool())
            .map(|b| if b { "yes" } else { "no" })
            .unwrap_or("—");

        rows.push(format!(
            "{}  {}  {}  {}  {}",
            fit(id, 36),
            fit(type_name, 12),
            fit(pan_or_acct, 20),
            fit(&exp, 7),
            fit(is_default, 7),
        ));
    }

    rows.join("\n")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── build_customer_body ───────────────────────────────────────────────────

    #[test]
    fn build_customer_body_all_fields() {
        let body = build_customer_body(
            Some("Alice"),
            Some("Smith"),
            Some("Acme Corp"),
            Some("alice@example.com"),
            Some("5550001234"),
        );
        assert_eq!(body["firstName"], "Alice");
        assert_eq!(body["lastName"], "Smith");
        assert_eq!(body["companyName"], "Acme Corp");
        assert_eq!(body["email"], "alice@example.com");
        assert_eq!(body["mobilePhoneNumber"], "5550001234");
    }

    #[test]
    fn build_customer_body_only_email() {
        let body = build_customer_body(None, None, None, Some("bob@example.com"), None);
        assert_eq!(body["email"], "bob@example.com");
        assert!(body.get("firstName").is_none());
        assert!(body.get("lastName").is_none());
        assert!(body.get("companyName").is_none());
        assert!(body.get("mobilePhoneNumber").is_none());
    }

    #[test]
    fn build_customer_body_no_fields_is_empty_object() {
        let body = build_customer_body(None, None, None, None, None);
        assert!(body.as_object().unwrap().is_empty());
    }

    // ── build_add_card_body ───────────────────────────────────────────────────

    #[test]
    fn build_add_card_body_mm_yy_splits_correctly() {
        let body = build_add_card_body(None, "4111111111111111", "12/26", Some("123")).unwrap();
        assert_eq!(body["pan"], "4111111111111111");
        assert_eq!(body["expirationMonth"], 12u32);
        assert_eq!(body["expirationYear"], 2026u32);
        assert_eq!(body["securityCode"], "123");
        assert!(body.get("name").is_none());
    }

    #[test]
    fn build_add_card_body_mm_yyyy_splits_correctly() {
        let body =
            build_add_card_body(Some("My Card"), "5500005555555559", "03/2027", None).unwrap();
        assert_eq!(body["expirationMonth"], 3u32);
        assert_eq!(body["expirationYear"], 2027u32);
        assert_eq!(body["name"], "My Card");
        assert!(body.get("securityCode").is_none());
    }

    #[test]
    fn build_add_card_body_bad_exp_returns_err() {
        let result = build_add_card_body(None, "4111111111111111", "13/26", Some("999"));
        assert!(result.is_err());
    }

    // ── build_add_ach_body ────────────────────────────────────────────────────

    #[test]
    fn build_add_ach_body_checking_maps_to_1() {
        let body = build_add_ach_body(
            None,
            "123456789",
            "021000021",
            AccountTypeArg::Checking,
            None,
            None,
        );
        assert_eq!(body["accountNumber"], "123456789");
        assert_eq!(body["routingNumber"], "021000021");
        assert_eq!(body["accountType"], 1);
        assert!(body.get("name").is_none());
        assert!(body.get("accountHolderType").is_none());
        assert!(body.get("taxId").is_none());
    }

    #[test]
    fn build_add_ach_body_savings_maps_to_2() {
        let body = build_add_ach_body(
            Some("Business Account"),
            "987654321",
            "011000138",
            AccountTypeArg::Savings,
            Some(AccountHolderTypeArg::Business),
            Some("12-3456789"),
        );
        assert_eq!(body["accountType"], 2);
        assert_eq!(body["accountHolderType"], 1); // Business = 1
        assert_eq!(body["taxId"], "12-3456789");
        assert_eq!(body["name"], "Business Account");
    }

    #[test]
    fn build_add_ach_body_personal_holder_maps_to_2() {
        let body = build_add_ach_body(
            None,
            "111222333",
            "021000021",
            AccountTypeArg::Checking,
            Some(AccountHolderTypeArg::Personal),
            None,
        );
        assert_eq!(body["accountHolderType"], 2); // Personal = 2
    }

    // ── customer_table ────────────────────────────────────────────────────────

    #[test]
    fn customer_table_shows_all_fields() {
        let v = json!({
            "id": "cust-001",
            "firstName": "Alice",
            "lastName": "Smith",
            "email": "alice@example.com",
            "mobilePhoneNumber": "5550001234",
            "companyName": "Acme Corp",
            "createdOn": "2024-01-15T10:00:00Z"
        });
        let table = customer_table(&v);
        assert!(table.contains("cust-001"));
        assert!(table.contains("Alice"));
        assert!(table.contains("Smith"));
        assert!(table.contains("alice@example.com"));
        assert!(table.contains("5550001234"));
        assert!(table.contains("Acme Corp"));
        assert!(table.contains("2024-01-15"));
    }

    #[test]
    fn customer_table_missing_fields_show_dash() {
        let v = json!({ "id": "cust-002" });
        let table = customer_table(&v);
        assert!(table.contains("cust-002"));
        // All other fields absent → rendered as "—"
        assert!(table.contains('—'));
    }

    // ── customer_list_table ───────────────────────────────────────────────────

    #[test]
    fn customer_list_table_renders_header_and_rows() {
        let items = vec![
            json!({
                "id": "cust-001",
                "firstName": "Alice",
                "lastName": "Smith",
                "email": "alice@example.com",
                "mobilePhoneNumber": "5550001234",
                "createdOn": "2024-01-15T10:00:00Z"
            }),
            json!({
                "id": "cust-002",
                "firstName": "Bob",
                "lastName": "Jones",
                "email": "bob@example.com",
                "mobilePhoneNumber": null,
                "createdOn": "2024-02-20T08:00:00Z"
            }),
        ];
        let table = customer_list_table(&items);
        assert!(table.contains("ID"));
        assert!(table.contains("NAME"));
        assert!(table.contains("EMAIL"));
        assert!(table.contains("cust-001"));
        assert!(table.contains("Alice Smith"));
        assert!(table.contains("alice@example.com"));
        assert!(table.contains("2024-01-15"));
        assert!(table.contains("cust-002"));
        assert!(table.contains("Bob Jones"));
    }

    #[test]
    fn customer_list_table_empty_list_returns_header_only() {
        let table = customer_list_table(&[]);
        assert!(table.contains("ID"));
        assert!(table.contains("NAME"));
        // No data rows beyond header + separator
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 2); // header + separator
    }

    // ── payment_methods_table ─────────────────────────────────────────────────

    #[test]
    fn payment_methods_table_renders_card_row() {
        let items = vec![json!({
            "id": "pm-001",
            "typeName": "Visa",
            "panMask": "411111****1111",
            "expirationMonth": 12,
            "expirationYear": 2026,
            "isDefault": true
        })];
        let table = payment_methods_table(&items);
        assert!(table.contains("pm-001"));
        assert!(table.contains("Visa"));
        assert!(table.contains("411111"));
        assert!(table.contains("12/2026"));
        assert!(table.contains("yes"));
    }

    #[test]
    fn payment_methods_table_renders_ach_row() {
        let items = vec![json!({
            "id": "pm-ach-001",
            "typeName": "ACH",
            "accountNumber": "123456789",
            "isDefault": false
        })];
        let table = payment_methods_table(&items);
        assert!(table.contains("pm-ach-001"));
        assert!(table.contains("ACH"));
        assert!(table.contains("123456789"));
        assert!(table.contains("no"));
    }

    #[test]
    fn payment_methods_table_empty_returns_header_only() {
        let table = payment_methods_table(&[]);
        assert!(table.contains("ID"));
        assert!(table.contains("TYPE"));
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 2);
    }

    // ── render_payment_method (singular) ──────────────────────────────────────

    #[test]
    fn render_payment_method_table_shows_id_typename_pan() {
        let v = json!({
            "id": "pm-card-42",
            "typeName": "Visa",
            "panMask": "411111****1111",
            "expirationMonth": 9,
            "expirationYear": 2028,
            "isDefault": false
        });
        let table = payment_method_table(&v);
        assert!(table.contains("pm-card-42"), "must contain id");
        assert!(table.contains("Visa"), "must contain typeName");
        assert!(table.contains("411111****1111"), "must contain panMask");
        assert!(table.contains("09/2028"), "must contain formatted expiry");
        assert!(table.contains("no"), "must contain isDefault=no");
    }

    #[test]
    fn render_payment_method_table_ach_shows_account_number() {
        let v = json!({
            "id": "pm-ach-77",
            "typeName": "ACH",
            "accountNumber": "987654321",
            "isDefault": true
        });
        let table = payment_method_table(&v);
        assert!(table.contains("pm-ach-77"), "must contain id");
        assert!(table.contains("ACH"), "must contain typeName");
        assert!(table.contains("987654321"), "must contain accountNumber");
        assert!(table.contains("yes"), "must contain isDefault=yes");
        // No expiry for ACH → dash
        assert!(table.contains('—'), "missing exp must render as —");
    }

    #[test]
    fn render_payment_method_quiet_returns_just_id() {
        let v = json!({
            "id": "pm-quiet-99",
            "typeName": "ACH",
            "accountNumber": "987654321"
        });
        // Quiet path: only the id string
        assert_eq!(payment_method_quiet(&v), Some("pm-quiet-99"));
    }

    #[test]
    fn render_payment_method_quiet_returns_none_when_no_id() {
        let v = json!({ "typeName": "Visa" });
        assert_eq!(payment_method_quiet(&v), None);
    }

    // ── extract_customer_items ────────────────────────────────────────────────

    #[test]
    fn extract_items_from_page_wrapper() {
        let v = json!({
            "items": [{"id": "c1"}, {"id": "c2"}],
            "total": 42
        });
        let (items, total) = extract_customer_items(&v);
        assert_eq!(items.len(), 2);
        assert_eq!(total, 42);
    }

    #[test]
    fn extract_items_falls_back_to_array() {
        let v = json!([{"id": "c1"}, {"id": "c2"}, {"id": "c3"}]);
        let (items, total) = extract_customer_items(&v);
        assert_eq!(items.len(), 3);
        assert_eq!(total, 3);
    }
}
