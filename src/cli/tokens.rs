//! CLI body builders and render helpers for the ISV Tokens command group
//! (`flute tokens …`).
//!
//! All body builders and render helpers are **pure functions** — no I/O, no
//! network — so they are trivially unit-testable with golden assertions.
//!
//! ## IMPORTANT: `tokens create` response contains `clientSecret` shown ONLY ONCE.
//! The json render path is lossless (full Envelope of the create response).
//! Table mode prints both `clientId` and `clientSecret` with a one-shot warning.

use serde_json::{Map, Value};

use crate::cli::output::{Envelope, OutputFormat, fit, prefix_chars};

// ── Body builders ─────────────────────────────────────────────────────────────

/// Build the JSON request body for `tokens create`.
///
/// Both `merchant_id` and `token_name` are required by the API.
///
/// # Field mapping
/// | Arg           | Wire key      |
/// |---------------|---------------|
/// | `merchant_id` | `merchantId`  |
/// | `token_name`  | `tokenName`   |
///
/// This is a **pure function** — no I/O, no network.
pub fn build_token_body(merchant_id: &str, token_name: &str) -> Value {
    let mut obj = Map::new();
    obj.insert("merchantId".into(), Value::String(merchant_id.to_string()));
    obj.insert("tokenName".into(), Value::String(token_name.to_string()));
    Value::Object(obj)
}

// ── Render helpers ────────────────────────────────────────────────────────────

/// Extract tokens array from a `{tokens: [...]}` response (defensive).
fn extract_tokens(v: &Value) -> Vec<Value> {
    if let Some(arr) = v.get("tokens").and_then(|x| x.as_array()) {
        arr.clone()
    } else if let Some(arr) = v.as_array() {
        arr.clone()
    } else {
        Vec::new()
    }
}

/// Build the table string for a token create response (pure helper, golden-testable).
///
/// Displays clientId, clientSecret, and a one-shot warning.
pub(crate) fn token_create_table(v: &Value) -> String {
    let get_str = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("—");
    format!(
        "clientId:     {}\nclientSecret: {}\n\nNOTE: The clientSecret is shown ONLY ONCE. Store it securely.",
        get_str("clientId"),
        get_str("clientSecret"),
    )
}

/// Build the table string for a token list (pure helper, golden-testable).
///
/// Columns: CLIENT ID(36), NAME(28), MERCHANT(36), CREATED(10)
pub(crate) fn token_list_table(items: &[Value]) -> String {
    let header = format!(
        "{:<36}  {:<28}  {:<36}  {:<10}",
        "CLIENT ID", "NAME", "MERCHANT", "CREATED"
    );
    let separator = "-".repeat(36 + 2 + 28 + 2 + 36 + 2 + 10);
    let mut rows = vec![header, separator];

    for item in items {
        let client_id = item.get("clientId").and_then(|v| v.as_str()).unwrap_or("—");
        let name = item
            .get("tokenName")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let merchant = item
            .get("merchantId")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let created = item
            .get("creationDate")
            .and_then(|v| v.as_str())
            .map(|s| prefix_chars(s, 10))
            .unwrap_or("—");

        rows.push(format!(
            "{}  {}  {}  {}",
            fit(client_id, 36),
            fit(name, 28),
            fit(merchant, 36),
            fit(created, 10),
        ));
    }

    rows.join("\n")
}

/// Render a token create response (lossless — contains one-shot clientSecret).
///
/// - `json`  → `Envelope { object: "api_token", data: v, … }` (lossless!)
/// - `table` → clientId + clientSecret + one-shot warning
/// - `quiet` → just the `clientId`
pub fn render_token_create(v: &Value, fmt: OutputFormat, environment: &str) -> anyhow::Result<()> {
    match fmt {
        OutputFormat::Json => {
            let env = Envelope::new("api_token", v.clone(), environment, None);
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        OutputFormat::Table => {
            println!("{}", token_create_table(v));
        }
        OutputFormat::Quiet => {
            if let Some(id) = v.get("clientId").and_then(|x| x.as_str()) {
                println!("{id}");
            }
        }
    }
    Ok(())
}

/// Render a token list response.
///
/// The response is `{tokens: [...]}` — reads the `tokens` array defensively.
///
/// - `json`  → `Envelope { object: "api_token_list", data: v, … }`
/// - `table` → columnar table via [`token_list_table`]
/// - `quiet` → one `clientId` per line
pub fn render_token_list(v: &Value, fmt: OutputFormat, environment: &str) -> anyhow::Result<()> {
    let items = extract_tokens(v);

    match fmt {
        OutputFormat::Json => {
            let env = Envelope::new("api_token_list", v.clone(), environment, None);
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        OutputFormat::Table => {
            println!("{}", token_list_table(&items));
        }
        OutputFormat::Quiet => {
            for item in &items {
                if let Some(id) = item.get("clientId").and_then(|x| x.as_str()) {
                    println!("{id}");
                }
            }
        }
    }
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── build_token_body ──────────────────────────────────────────────────────

    #[test]
    fn build_token_body_sets_merchant_id_and_token_name() {
        let body = build_token_body("merchant-001", "My App Token");
        assert_eq!(body["merchantId"], "merchant-001");
        assert_eq!(body["tokenName"], "My App Token");
    }

    #[test]
    fn build_token_body_has_exactly_two_keys() {
        let body = build_token_body("m-123", "Test Token");
        let obj = body.as_object().unwrap();
        assert_eq!(
            obj.len(),
            2,
            "body must have exactly two keys: merchantId + tokenName"
        );
    }

    #[test]
    fn build_token_body_uses_correct_wire_keys() {
        let body = build_token_body("m-456", "Token Name");
        assert!(
            body.get("merchantId").is_some(),
            "merchantId must be present"
        );
        assert!(body.get("tokenName").is_some(), "tokenName must be present");
        // Wire keys must NOT be in camelCase variants like "merchant_id"
        assert!(body.get("merchant_id").is_none());
        assert!(body.get("token_name").is_none());
    }

    // ── token_create_table ────────────────────────────────────────────────────

    #[test]
    fn token_create_table_shows_client_id_and_secret() {
        let v = json!({
            "clientId": "client-abc-123",
            "clientSecret": "super-secret-one-shot"
        });
        let table = token_create_table(&v);
        assert!(table.contains("client-abc-123"), "must contain clientId");
        assert!(
            table.contains("super-secret-one-shot"),
            "must contain clientSecret"
        );
        assert!(table.contains("ONLY ONCE"), "must contain one-shot warning");
    }

    #[test]
    fn token_create_table_missing_fields_show_dash() {
        let v = json!({});
        let table = token_create_table(&v);
        assert!(table.contains('—'), "missing fields must render as —");
        assert!(
            table.contains("ONLY ONCE"),
            "must still show one-shot warning"
        );
    }

    // ── token_list_table ──────────────────────────────────────────────────────

    #[test]
    fn token_list_table_renders_header_and_rows() {
        let items = vec![
            json!({
                "clientId": "client-abc-123",
                "tokenName": "My App Token",
                "merchantId": "merchant-001",
                "creationDate": "2024-03-15T10:00:00Z"
            }),
            json!({
                "clientId": "client-def-456",
                "tokenName": "Another Token",
                "merchantId": "merchant-002",
                "creationDate": "2024-04-20T08:00:00Z"
            }),
        ];
        let table = token_list_table(&items);
        assert!(table.contains("CLIENT ID"), "must contain CLIENT ID header");
        assert!(table.contains("NAME"), "must contain NAME header");
        assert!(table.contains("MERCHANT"), "must contain MERCHANT header");
        assert!(table.contains("CREATED"), "must contain CREATED header");
        assert!(
            table.contains("client-abc-123"),
            "must contain first clientId"
        );
        assert!(
            table.contains("My App Token"),
            "must contain first tokenName"
        );
        assert!(
            table.contains("merchant-001"),
            "must contain first merchantId"
        );
        assert!(
            table.contains("2024-03-15"),
            "must contain first date (10 chars)"
        );
        assert!(
            table.contains("client-def-456"),
            "must contain second clientId"
        );
    }

    #[test]
    fn token_list_table_empty_returns_header_only() {
        let table = token_list_table(&[]);
        assert!(table.contains("CLIENT ID"));
        assert!(table.contains("NAME"));
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 2); // header + separator
    }

    #[test]
    fn token_list_table_missing_fields_show_dash() {
        let items = vec![json!({ "clientId": "client-only" })];
        let table = token_list_table(&items);
        assert!(table.contains("client-only"));
        assert!(table.contains('—'));
    }

    // ── extract_tokens ────────────────────────────────────────────────────────

    #[test]
    fn extract_tokens_from_tokens_key() {
        let v = json!({
            "tokens": [
                { "clientId": "c1" },
                { "clientId": "c2" }
            ]
        });
        let items = extract_tokens(&v);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn extract_tokens_falls_back_to_array() {
        let v = json!([{ "clientId": "c1" }, { "clientId": "c2" }]);
        let items = extract_tokens(&v);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn extract_tokens_returns_empty_for_unknown_shape() {
        let v = json!({ "other": "data" });
        let items = extract_tokens(&v);
        assert!(items.is_empty());
    }
}
