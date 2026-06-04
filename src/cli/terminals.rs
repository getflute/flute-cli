//! CLI render helpers for the Terminals command group (`flute terminals …`).
//!
//! All render helpers are **pure functions** — no I/O, no network — so they
//! are trivially unit-testable with golden assertions.

use serde_json::{Value, json};

use crate::cli::output::{Envelope, OutputFormat, fit};

// ── Render helpers ────────────────────────────────────────────────────────────

/// Render a terminal list response (`PageOfGetIsvTerminalsResponseDto`).
///
/// The server wraps items in `{items, total}`.  Reads defensively:
/// - First tries `v["items"]` as an array (standard page wrapper).
/// - Falls back to treating `v` itself as an array.
///
/// - `json`  → `Envelope { object: "terminal_list", data: {items, total}, … }`
/// - `table` → columnar table via [`terminal_list_table`]
/// - `quiet` → one ID per line
pub fn render_terminal_list(v: &Value, fmt: OutputFormat, environment: &str) -> anyhow::Result<()> {
    let (items, total) = extract_items(v);

    match fmt {
        OutputFormat::Json => {
            let data = json!({ "items": items, "total": total });
            let env = Envelope::new("terminal_list", data, environment, None);
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        OutputFormat::Table => {
            println!("{}", terminal_list_table(&items));
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

/// Extract items array and total from a page response (defensive).
fn extract_items(v: &Value) -> (Vec<Value>, u64) {
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

/// Build the table string for a terminal list (pure helper, golden-testable).
///
/// Columns: ID (36), SERIAL (16), MODEL (20), MODE (20), CONNECTION (12), LAST SEEN (24)
pub(crate) fn terminal_list_table(items: &[Value]) -> String {
    let header = format!(
        "{:<36}  {:<16}  {:<20}  {:<20}  {:<12}  {:<24}",
        "ID", "SERIAL", "MODEL", "MODE", "CONNECTION", "LAST SEEN"
    );
    let separator = "-".repeat(36 + 2 + 16 + 2 + 20 + 2 + 20 + 2 + 12 + 2 + 24);
    let mut rows = vec![header, separator];

    for item in items {
        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("—");
        let serial = item
            .get("serialNumber")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let model = item
            .get("terminalModel")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let mode = item
            .get("terminalModeName")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let connection = item
            .get("connectionStatus")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let last_seen = item
            .get("lastSeenTimestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("—");

        rows.push(format!(
            "{}  {}  {}  {}  {}  {}",
            fit(id, 36),
            fit(serial, 16),
            fit(model, 20),
            fit(mode, 20),
            fit(connection, 12),
            fit(last_seen, 24),
        ));
    }

    rows.join("\n")
}

/// Build the table string for a single terminal status (pure helper, golden-testable).
///
/// Renders: terminalId, terminalPosStatus, connectionStatus, connectionType,
/// batteryLevel, wifiConnectionStrength, availabilityStatus, printerStatus,
/// lastSeenTimestamp — with "—" fallbacks for absent fields.
pub(crate) fn terminal_status_table(v: &Value) -> String {
    let get = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("—");
    let get_num = |k: &str| {
        v.get(k)
            .and_then(|x| x.as_u64())
            .map(|n| n.to_string())
            .unwrap_or_else(|| "—".to_string())
    };

    format!(
        "terminalId:             {}\nterminalPosStatus:      {}\nconnectionStatus:       {}\nconnectionType:         {}\nbatteryLevel:           {}\nwifiConnectionStrength: {}\navailabilityStatus:     {}\nprinterStatus:          {}\nlastSeenTimestamp:      {}",
        get("terminalId"),
        get("terminalPosStatus"),
        get("connectionStatus"),
        get("connectionType"),
        get_num("batteryLevel"),
        get_num("wifiConnectionStrength"),
        get("availabilityStatus"),
        get("printerStatus"),
        get("lastSeenTimestamp"),
    )
}

/// Render a single terminal status response.
///
/// - `json`  → `Envelope { object: "terminal_status", data: v, … }`
/// - `table` → key-value list via [`terminal_status_table`]
/// - `quiet` → just the terminal `terminalId`
pub fn render_terminal_status(
    v: &Value,
    fmt: OutputFormat,
    environment: &str,
) -> anyhow::Result<()> {
    match fmt {
        OutputFormat::Json => {
            let env = Envelope::new("terminal_status", v.clone(), environment, None);
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        OutputFormat::Table => {
            println!("{}", terminal_status_table(v));
        }
        OutputFormat::Quiet => {
            // prefer terminalId; fall back to id
            let id = v
                .get("terminalId")
                .or_else(|| v.get("id"))
                .and_then(|x| x.as_str())
                .unwrap_or("—");
            println!("{id}");
        }
    }
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_item() -> Value {
        json!({
            "id": "term-001",
            "serialNumber": "SN123456",
            "terminalManufacturer": "Ingenico",
            "terminalModel": "Desk/5000",
            "terminalModeName": "Desk 5000",
            "connectionStatus": "Online",
            "deliveryStatusName": "Delivered",
            "lastSeenTimestamp": "2024-06-01T10:00:00Z"
        })
    }

    fn sample_status() -> Value {
        json!({
            "terminalId": "term-001",
            "terminalPosStatus": "Active",
            "connectionStatus": "Online",
            "connectionType": "WiFi",
            "wifiConnectionStrength": 85,
            "batteryLevel": 72,
            "availabilityStatus": "Available",
            "ariseTerminalVersion": "2.1.0",
            "printerStatus": "Ready",
            "lastSeenTimestamp": "2024-06-01T10:00:00Z"
        })
    }

    // ── terminal_list_table ───────────────────────────────────────────────────

    #[test]
    fn terminal_list_table_renders_header_and_rows() {
        let items = vec![sample_item()];
        let table = terminal_list_table(&items);
        assert!(table.contains("ID"), "must contain ID header");
        assert!(table.contains("SERIAL"), "must contain SERIAL header");
        assert!(table.contains("MODEL"), "must contain MODEL header");
        assert!(
            table.contains("CONNECTION"),
            "must contain CONNECTION header"
        );
        assert!(table.contains("LAST SEEN"), "must contain LAST SEEN header");
        assert!(table.contains("term-001"), "must contain item id");
        assert!(table.contains("SN123456"), "must contain serial number");
        assert!(table.contains("Desk/5000"), "must contain model");
        assert!(table.contains("Desk 5000"), "must contain mode name");
        assert!(table.contains("Online"), "must contain connection status");
    }

    #[test]
    fn terminal_list_table_empty_returns_header_only() {
        let table = terminal_list_table(&[]);
        assert!(table.contains("ID"));
        assert!(table.contains("SERIAL"));
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 2); // header + separator
    }

    #[test]
    fn terminal_list_table_missing_fields_show_dash() {
        let items = vec![json!({ "id": "term-002" })];
        let table = terminal_list_table(&items);
        assert!(table.contains("term-002"));
        assert!(table.contains('—'));
    }

    // ── terminal_status_table ─────────────────────────────────────────────────

    #[test]
    fn terminal_status_table_shows_all_key_fields() {
        let table = terminal_status_table(&sample_status());
        assert!(table.contains("term-001"), "must contain terminalId");
        assert!(table.contains("Active"), "must contain terminalPosStatus");
        assert!(table.contains("Online"), "must contain connectionStatus");
        assert!(table.contains("WiFi"), "must contain connectionType");
        assert!(table.contains("72"), "must contain batteryLevel");
        assert!(table.contains("85"), "must contain wifiConnectionStrength");
        assert!(
            table.contains("Available"),
            "must contain availabilityStatus"
        );
        assert!(table.contains("Ready"), "must contain printerStatus");
        assert!(
            table.contains("2024-06-01"),
            "must contain lastSeenTimestamp"
        );
    }

    #[test]
    fn terminal_status_table_missing_fields_show_dash() {
        let v = json!({ "terminalId": "term-999" });
        let table = terminal_status_table(&v);
        assert!(table.contains("term-999"));
        assert!(table.contains('—'));
    }

    // ── extract_items ─────────────────────────────────────────────────────────

    #[test]
    fn extract_items_from_page_wrapper() {
        let v = json!({
            "items": [{ "id": "t1" }, { "id": "t2" }],
            "total": 42
        });
        let (items, total) = extract_items(&v);
        assert_eq!(items.len(), 2);
        assert_eq!(total, 42);
    }

    #[test]
    fn extract_items_falls_back_to_array() {
        let v = json!([{ "id": "t1" }, { "id": "t2" }, { "id": "t3" }]);
        let (items, total) = extract_items(&v);
        assert_eq!(items.len(), 3);
        assert_eq!(total, 3);
    }
}
