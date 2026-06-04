//! CLI body builders and render helpers for the Devices command group
//! (`flute devices …`).
//!
//! All body builders and render helpers are **pure functions** — no I/O, no
//! network — so they are trivially unit-testable with golden assertions.
//!
//! ## DeviceResponseDto field names (inferred defensively)
//! The OpenAPI spec names the container `DeviceResponseDto` but does not list
//! all fields in the reference doc.  Based on the wire-format doc and common
//! API conventions we render: `id`, `deviceId`, `deviceName`, `status`.
//! Additional fields are surfaced as-is in JSON output via the Envelope.
//! Field names were **inferred defensively** — live testing may reveal extra
//! fields; add them to the table helpers as discovered.

use serde_json::{Map, Value, json};

use crate::cli::output::{Envelope, OutputFormat, fit};

// ── Body builders ─────────────────────────────────────────────────────────────

/// Build the JSON request body for `devices register <id>`.
///
/// POST `/pay-api/v1/devices` is create-or-update:
/// `{deviceId: <id>, deviceName?: <name>}`.
///
/// - `device_id` → `deviceId` (required)
/// - `name`      → `deviceName` (omitted if `None`)
///
/// This is a **pure function** — no I/O, no network.
pub fn build_register_device_body(device_id: &str, name: Option<&str>) -> Value {
    let mut obj = Map::new();
    obj.insert("deviceId".into(), Value::String(device_id.to_string()));
    if let Some(n) = name {
        obj.insert("deviceName".into(), Value::String(n.to_string()));
    }
    Value::Object(obj)
}

/// Build the JSON request body for `devices ttp-jwt --device-id <id>`.
///
/// POST `/pay-api/v1/devices/tap-to-pay/jwt`:
/// `{deviceId: <id>}`.
///
/// This is a **pure function** — no I/O, no network.
pub fn build_ttp_jwt_body(device_id: &str) -> Value {
    json!({ "deviceId": device_id })
}

// ── Render helpers ────────────────────────────────────────────────────────────

/// Build the table string for a device list (pure helper, golden-testable).
///
/// The response is `{devices: [...]}` — reads the `devices` array defensively.
/// Columns: ID (36), DEVICE ID (32), NAME (24), STATUS (16)
///
/// Field names are inferred defensively from `DeviceResponseDto`; live testing
/// may surface additional fields — extend this helper as needed.
pub(crate) fn device_list_table(items: &[Value]) -> String {
    let header = format!(
        "{:<36}  {:<32}  {:<24}  {:<16}",
        "ID", "DEVICE ID", "NAME", "STATUS"
    );
    let separator = "-".repeat(36 + 2 + 32 + 2 + 24 + 2 + 16);
    let mut rows = vec![header, separator];

    for item in items {
        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("—");
        let device_id = item.get("deviceId").and_then(|v| v.as_str()).unwrap_or("—");
        let name = item
            .get("deviceName")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("—");

        rows.push(format!(
            "{}  {}  {}  {}",
            fit(id, 36),
            fit(device_id, 32),
            fit(name, 24),
            fit(status, 16),
        ));
    }

    rows.join("\n")
}

/// Build the table string for a single device (pure helper, golden-testable).
pub(crate) fn device_table(v: &Value) -> String {
    let get = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("—");
    format!(
        "id:         {}\ndeviceId:   {}\ndeviceName: {}\nstatus:     {}",
        get("id"),
        get("deviceId"),
        get("deviceName"),
        get("status"),
    )
}

/// Extract the devices array from a `{devices: [...]}` response (defensive).
///
/// Falls back to treating the value as an array directly.
fn extract_devices(v: &Value) -> Vec<Value> {
    if let Some(arr) = v.get("devices").and_then(|x| x.as_array()) {
        arr.clone()
    } else if let Some(arr) = v.as_array() {
        arr.clone()
    } else {
        Vec::new()
    }
}

/// Render a device list response (`GetIsvDevicesResponseDto`).
///
/// - `json`  → `Envelope { object: "device_list", data: v, … }`
/// - `table` → columnar table via [`device_list_table`]
/// - `quiet` → one ID per line (uses `id` field; falls back to `deviceId`)
pub fn render_device_list(v: &Value, fmt: OutputFormat, environment: &str) -> anyhow::Result<()> {
    let items = extract_devices(v);

    match fmt {
        OutputFormat::Json => {
            let env = Envelope::new("device_list", v.clone(), environment, None);
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        OutputFormat::Table => {
            println!("{}", device_list_table(&items));
        }
        OutputFormat::Quiet => {
            for item in &items {
                let id = item
                    .get("id")
                    .or_else(|| item.get("deviceId"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("—");
                println!("{id}");
            }
        }
    }
    Ok(())
}

/// Render a single device response.
///
/// - `json`  → `Envelope { object: "device", data: v, … }`
/// - `table` → key-value list via [`device_table`]
/// - `quiet` → just the `id` (or `deviceId` as fallback)
pub fn render_device(v: &Value, fmt: OutputFormat, environment: &str) -> anyhow::Result<()> {
    match fmt {
        OutputFormat::Json => {
            let env = Envelope::new("device", v.clone(), environment, None);
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        OutputFormat::Table => {
            println!("{}", device_table(v));
        }
        OutputFormat::Quiet => {
            let id = v
                .get("id")
                .or_else(|| v.get("deviceId"))
                .and_then(|x| x.as_str())
                .unwrap_or("—");
            println!("{id}");
        }
    }
    Ok(())
}

/// Render a Tap-to-Pay JWT response.
///
/// - `json`  → `Envelope { object: "tap_to_pay_jwt", data: v, … }`
/// - `table` → key-value list: jwt (truncated for display), deviceId
/// - `quiet` → the `jwt` token value if present, else `id`, else `deviceId`
pub fn render_ttp_jwt(v: &Value, fmt: OutputFormat, environment: &str) -> anyhow::Result<()> {
    match fmt {
        OutputFormat::Json => {
            let env = Envelope::new("tap_to_pay_jwt", v.clone(), environment, None);
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        OutputFormat::Table => {
            println!("{}", ttp_jwt_table(v));
        }
        OutputFormat::Quiet => {
            // Prefer the jwt token; fall back to id or deviceId
            let token = v
                .get("jwt")
                .or_else(|| v.get("token"))
                .or_else(|| v.get("id"))
                .or_else(|| v.get("deviceId"))
                .and_then(|x| x.as_str())
                .unwrap_or("—");
            println!("{token}");
        }
    }
    Ok(())
}

/// Build the table string for a TTP JWT response (pure helper).
pub(crate) fn ttp_jwt_table(v: &Value) -> String {
    let get = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("—");
    // JWT values can be very long — render them as-is (table is for humans)
    format!("jwt:      {}\ndeviceId: {}", get("jwt"), get("deviceId"),)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── build_register_device_body ────────────────────────────────────────────

    #[test]
    fn build_register_device_body_with_name() {
        let body = build_register_device_body("DEVICE-ABC-123", Some("Register 1"));
        assert_eq!(body["deviceId"], "DEVICE-ABC-123");
        assert_eq!(body["deviceName"], "Register 1");
    }

    #[test]
    fn build_register_device_body_without_name_omits_device_name() {
        let body = build_register_device_body("DEVICE-ABC-123", None);
        assert_eq!(body["deviceId"], "DEVICE-ABC-123");
        assert!(
            body.get("deviceName").is_none(),
            "deviceName must be absent when name is None"
        );
    }

    #[test]
    fn build_register_device_body_only_device_id_key_present() {
        let body = build_register_device_body("MY-DEVICE", None);
        let obj = body.as_object().unwrap();
        assert_eq!(obj.len(), 1, "only deviceId key must be present");
        assert_eq!(body["deviceId"], "MY-DEVICE");
    }

    // ── build_ttp_jwt_body ────────────────────────────────────────────────────

    #[test]
    fn build_ttp_jwt_body_has_device_id() {
        let body = build_ttp_jwt_body("DEVICE-XYZ");
        assert_eq!(body["deviceId"], "DEVICE-XYZ");
    }

    #[test]
    fn build_ttp_jwt_body_has_exactly_one_key() {
        let body = build_ttp_jwt_body("DEVICE-XYZ");
        let obj = body.as_object().unwrap();
        assert_eq!(obj.len(), 1, "body must have exactly one key: deviceId");
    }

    // ── device_list_table ─────────────────────────────────────────────────────

    #[test]
    fn device_list_table_renders_header_and_rows() {
        let items = vec![json!({
            "id": "dev-001",
            "deviceId": "DEVICE-ABC-123",
            "deviceName": "Register 1",
            "status": "Active"
        })];
        let table = device_list_table(&items);
        assert!(table.contains("ID"), "must contain ID header");
        assert!(table.contains("DEVICE ID"), "must contain DEVICE ID header");
        assert!(table.contains("NAME"), "must contain NAME header");
        assert!(table.contains("STATUS"), "must contain STATUS header");
        assert!(table.contains("dev-001"), "must contain item id");
        assert!(table.contains("DEVICE-ABC-123"), "must contain deviceId");
        assert!(table.contains("Register 1"), "must contain deviceName");
        assert!(table.contains("Active"), "must contain status");
    }

    #[test]
    fn device_list_table_empty_returns_header_only() {
        let table = device_list_table(&[]);
        assert!(table.contains("ID"));
        assert!(table.contains("DEVICE ID"));
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 2); // header + separator
    }

    #[test]
    fn device_list_table_missing_fields_show_dash() {
        let items = vec![json!({ "id": "dev-002" })];
        let table = device_list_table(&items);
        assert!(table.contains("dev-002"));
        assert!(table.contains('—'));
    }

    // ── device_table ──────────────────────────────────────────────────────────

    #[test]
    fn device_table_shows_all_fields() {
        let v = json!({
            "id": "dev-001",
            "deviceId": "DEVICE-ABC-123",
            "deviceName": "Register 1",
            "status": "Active"
        });
        let table = device_table(&v);
        assert!(table.contains("dev-001"));
        assert!(table.contains("DEVICE-ABC-123"));
        assert!(table.contains("Register 1"));
        assert!(table.contains("Active"));
    }

    #[test]
    fn device_table_missing_fields_show_dash() {
        let v = json!({ "id": "dev-999" });
        let table = device_table(&v);
        assert!(table.contains("dev-999"));
        assert!(table.contains('—'));
    }

    // ── ttp_jwt_table ─────────────────────────────────────────────────────────

    #[test]
    fn ttp_jwt_table_shows_jwt_and_device_id() {
        let v = json!({
            "jwt": "eyJhbGciOiJSUzI1NiJ9.test.sig",
            "deviceId": "DEVICE-ABC-123"
        });
        let table = ttp_jwt_table(&v);
        assert!(table.contains("eyJhbGciOiJSUzI1NiJ9"));
        assert!(table.contains("DEVICE-ABC-123"));
    }

    #[test]
    fn ttp_jwt_table_missing_fields_show_dash() {
        let v = json!({});
        let table = ttp_jwt_table(&v);
        assert!(table.contains('—'));
    }

    // ── extract_devices ───────────────────────────────────────────────────────

    #[test]
    fn extract_devices_from_devices_key() {
        let v = json!({
            "devices": [
                { "id": "d1" },
                { "id": "d2" }
            ]
        });
        let items = extract_devices(&v);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn extract_devices_falls_back_to_array() {
        let v = json!([{ "id": "d1" }, { "id": "d2" }, { "id": "d3" }]);
        let items = extract_devices(&v);
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn extract_devices_returns_empty_for_unknown_shape() {
        let v = json!({ "other": "data" });
        let items = extract_devices(&v);
        assert!(items.is_empty());
    }
}
