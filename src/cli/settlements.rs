//! CLI body builders and render helpers for the Settlements command group
//! (`flute settlements …`).
//!
//! All render helpers are **pure functions** — no I/O, no network — so they
//! are trivially unit-testable with golden assertions.
//!
//! ## Note on `settlements get <id>`
//! There is no single-batch endpoint in the API.  `get` fetches a page (with a
//! larger pageSize, e.g. 100) and filters client-side by `id`.  It is
//! page-bounded by design.

use serde_json::Value;

use crate::cli::output::{Envelope, OutputFormat, fit};

// ── Render helpers ────────────────────────────────────────────────────────────

/// Extract `items` array and `total` from a page response (defensive).
fn extract_settlement_items(v: &Value) -> (Vec<Value>, u64) {
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

/// Format a monetary amount as a 2-decimal string, or "—" if absent.
fn fmt_amount(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_f64())
        .map(|f| format!("{f:.2}"))
        .unwrap_or_else(|| "—".to_string())
}

/// Build the table string for a settlement batch list (pure helper, golden-testable).
///
/// Columns: ID(36), PROCESSOR(20), BATCH DATE(10), TXNS(6), SALES(12), REFUNDS(12), NET(12), STATUS(10)
pub(crate) fn settlement_list_table(items: &[Value]) -> String {
    let header = format!(
        "{:<36}  {:<20}  {:<10}  {:<6}  {:<12}  {:<12}  {:<12}  {:<10}",
        "ID", "PROCESSOR", "BATCH DATE", "TXNS", "SALES", "REFUNDS", "NET", "STATUS"
    );
    let separator = "-".repeat(36 + 2 + 20 + 2 + 10 + 2 + 6 + 2 + 12 + 2 + 12 + 2 + 12 + 2 + 10);
    let mut rows = vec![header, separator];

    for item in items {
        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("—");
        let processor = item
            .get("paymentProcessorName")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let batch_date = item
            .get("batchDateTime")
            .and_then(|v| v.as_str())
            .map(|s| if s.len() >= 10 { &s[..10] } else { s })
            .unwrap_or("—");
        let txns = item
            .get("transactionCount")
            .and_then(|v| v.as_u64())
            .map(|n| n.to_string())
            .unwrap_or_else(|| "—".to_string());
        let sales = fmt_amount(item, "salesAmount");
        let refunds = fmt_amount(item, "refundsAmount");
        let net = fmt_amount(item, "netAmount");
        let status = item
            .get("statusName")
            .and_then(|v| v.as_str())
            .unwrap_or("—");

        rows.push(format!(
            "{}  {}  {}  {}  {}  {}  {}  {}",
            fit(id, 36),
            fit(processor, 20),
            fit(batch_date, 10),
            fit(&txns, 6),
            fit(&sales, 12),
            fit(&refunds, 12),
            fit(&net, 12),
            fit(status, 10),
        ));
    }

    rows.join("\n")
}

/// Build the table string for a single settlement batch (pure helper, golden-testable).
pub(crate) fn settlement_table(v: &Value) -> String {
    let get_str = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("—");
    let txns = v
        .get("transactionCount")
        .and_then(|x| x.as_u64())
        .map(|n| n.to_string())
        .unwrap_or_else(|| "—".to_string());
    format!(
        "id:                   {}\npaymentProcessorName: {}\nbatchDateTime:        {}\ntransactionCount:     {}\nsalesAmount:          {}\nrefundsAmount:        {}\nnetAmount:            {}\nstatusName:           {}",
        get_str("id"),
        get_str("paymentProcessorName"),
        get_str("batchDateTime"),
        txns,
        fmt_amount(v, "salesAmount"),
        fmt_amount(v, "refundsAmount"),
        fmt_amount(v, "netAmount"),
        get_str("statusName"),
    )
}

/// Render a settlement batch list response.
///
/// - `json`  → `Envelope { object: "settlement_list", data: {items,total}, … }`
/// - `table` → columnar table via [`settlement_list_table`]
/// - `quiet` → one `id` per line
pub fn render_settlement_list(
    v: &Value,
    fmt: OutputFormat,
    environment: &str,
) -> anyhow::Result<()> {
    let (items, total) = extract_settlement_items(v);

    match fmt {
        OutputFormat::Json => {
            let data = serde_json::json!({ "items": items, "total": total });
            let env = Envelope::new("settlement_list", data, environment, None);
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        OutputFormat::Table => {
            println!("{}", settlement_list_table(&items));
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

/// Render a single settlement batch.
///
/// - `json`  → `Envelope { object: "settlement", data: v, … }`
/// - `table` → key-value list via [`settlement_table`]
/// - `quiet` → just the `id`
pub fn render_settlement(v: &Value, fmt: OutputFormat, environment: &str) -> anyhow::Result<()> {
    match fmt {
        OutputFormat::Json => {
            let env = Envelope::new("settlement", v.clone(), environment, None);
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        OutputFormat::Table => {
            println!("{}", settlement_table(v));
        }
        OutputFormat::Quiet => {
            if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
                println!("{id}");
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

    fn sample_batch() -> serde_json::Value {
        json!({
            "id": "batch-001",
            "paymentProcessorName": "TSYS",
            "externalBatchId": "EXT-001",
            "batchDateTime": "2024-03-15T10:00:00Z",
            "transactionCount": 42,
            "salesAmount": 1500.00,
            "refundsAmount": 50.00,
            "netAmount": 1450.00,
            "statusId": 2,
            "statusName": "Settled"
        })
    }

    // ── settlement_list_table ─────────────────────────────────────────────────

    #[test]
    fn settlement_list_table_renders_header_and_rows() {
        let items = vec![sample_batch()];
        let table = settlement_list_table(&items);
        assert!(table.contains("ID"), "must contain ID header");
        assert!(table.contains("PROCESSOR"), "must contain PROCESSOR header");
        assert!(
            table.contains("BATCH DATE"),
            "must contain BATCH DATE header"
        );
        assert!(table.contains("TXNS"), "must contain TXNS header");
        assert!(table.contains("SALES"), "must contain SALES header");
        assert!(table.contains("REFUNDS"), "must contain REFUNDS header");
        assert!(table.contains("NET"), "must contain NET header");
        assert!(table.contains("STATUS"), "must contain STATUS header");
        assert!(table.contains("batch-001"), "must contain id");
        assert!(table.contains("TSYS"), "must contain processor name");
        assert!(
            table.contains("2024-03-15"),
            "must contain date (first 10 chars)"
        );
        assert!(table.contains("42"), "must contain transaction count");
        assert!(table.contains("1500.00"), "must contain sales amount");
        assert!(table.contains("50.00"), "must contain refunds amount");
        assert!(table.contains("1450.00"), "must contain net amount");
        assert!(table.contains("Settled"), "must contain status name");
    }

    #[test]
    fn settlement_list_table_empty_returns_header_only() {
        let table = settlement_list_table(&[]);
        assert!(table.contains("ID"));
        assert!(table.contains("PROCESSOR"));
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 2); // header + separator
    }

    #[test]
    fn settlement_list_table_missing_fields_show_dash() {
        let items = vec![json!({ "id": "batch-002" })];
        let table = settlement_list_table(&items);
        assert!(table.contains("batch-002"));
        assert!(table.contains('—'));
    }

    #[test]
    fn settlement_list_table_amounts_use_two_decimal_places() {
        let items = vec![json!({
            "id": "batch-003",
            "salesAmount": 100.5,
            "refundsAmount": 0.0,
            "netAmount": 100.5
        })];
        let table = settlement_list_table(&items);
        assert!(table.contains("100.50"), "amounts must be 2dp");
        assert!(table.contains("0.00"), "zero amounts must be 2dp");
    }

    // ── settlement_table (single) ─────────────────────────────────────────────

    #[test]
    fn settlement_table_shows_all_fields() {
        let v = sample_batch();
        let table = settlement_table(&v);
        assert!(table.contains("batch-001"), "must contain id");
        assert!(table.contains("TSYS"), "must contain processor");
        assert!(table.contains("2024-03-15"), "must contain date");
        assert!(table.contains("42"), "must contain txn count");
        assert!(table.contains("1500.00"), "must contain sales");
        assert!(table.contains("50.00"), "must contain refunds");
        assert!(table.contains("1450.00"), "must contain net");
        assert!(table.contains("Settled"), "must contain status");
    }

    #[test]
    fn settlement_table_missing_fields_show_dash() {
        let v = json!({ "id": "batch-999" });
        let table = settlement_table(&v);
        assert!(table.contains("batch-999"));
        assert!(table.contains('—'));
    }

    // ── extract_settlement_items ──────────────────────────────────────────────

    #[test]
    fn extract_items_from_page_wrapper() {
        let v = json!({
            "items": [{"id": "b1"}, {"id": "b2"}],
            "total": 42
        });
        let (items, total) = extract_settlement_items(&v);
        assert_eq!(items.len(), 2);
        assert_eq!(total, 42);
    }

    #[test]
    fn extract_items_falls_back_to_array() {
        let v = json!([{"id": "b1"}, {"id": "b2"}, {"id": "b3"}]);
        let (items, total) = extract_settlement_items(&v);
        assert_eq!(items.len(), 3);
        assert_eq!(total, 3);
    }

    #[test]
    fn extract_items_returns_empty_for_unknown_shape() {
        let v = json!({ "other": "data" });
        let (items, total) = extract_settlement_items(&v);
        assert!(items.is_empty());
        assert_eq!(total, 0);
    }
}
