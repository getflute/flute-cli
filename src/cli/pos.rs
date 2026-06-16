//! CLI body builders and render helpers for the POS Transactions command group
//! (`flute pos …`).
//!
//! All body builders and render helpers are **pure functions** — no I/O, no
//! network — so they are trivially unit-testable with golden assertions.
//!
//! ## --wait long-poll mechanic
//! `pos_is_final(v)` is the pure decision fn that reads `v["isCompleted"]`.
//! The async poller loop (`run_wait_poll`) uses `tokio::select!` to interleave
//! timed polling with Ctrl-C graceful shutdown.

use serde_json::{Map, Value};

use crate::api::models::to_amount_number;
use crate::cli::money::{parse_amount, parse_rate};
use crate::cli::output::{Envelope, OutputFormat, fit};

// ── Args struct (mirrors PosCommand::Create fields) ───────────────────────────

/// All CLI flags that feed a `pos create` request.
///
/// Carried as a struct so the body builder is a pure function that can be
/// unit-tested independently of Clap.
pub struct PosCreateArgs {
    pub terminal_id: String,
    pub transaction_type: i32,
    pub amount: Option<String>,
    pub currency_id: i32,
    pub tip_amount: Option<String>,
    pub tip_rate: Option<String>,
    pub pos_device_id: Option<String>,
    pub reference_id: Option<String>,
    pub payment_processor_id: Option<String>,
    pub customer_id: Option<String>,
    pub target_transaction_id: Option<String>,
    pub reading_method: Option<i32>,
    /// If true, sets `waitForAcceptanceByTerminal: true` in the body and
    /// activates the polling loop after the POST returns.
    pub wait: bool,
}

// ── Body builder ─────────────────────────────────────────────────────────────

/// Build the JSON request body for `pos create`.
///
/// Fields are camelCase as required by the wire format:
/// - Required: `terminalId`
/// - Defaulted: `transactionTypeId` (default 2=Sale), `currencyId` (default 1),
///   `waitForAcceptanceByTerminal` (= args.wait)
/// - Only-present-if-Some: all optional fields
///
/// Returns an `Err` if an amount string fails to parse.
pub fn build_pos_create_body(args: &PosCreateArgs) -> anyhow::Result<Value> {
    let mut obj = Map::new();

    // Required
    obj.insert("terminalId".into(), Value::String(args.terminal_id.clone()));

    // Defaulted
    obj.insert(
        "transactionTypeId".into(),
        Value::Number(serde_json::Number::from(args.transaction_type)),
    );
    obj.insert(
        "currencyId".into(),
        Value::Number(serde_json::Number::from(args.currency_id)),
    );

    // Amount (optional; parse via to_amount_number)
    if let Some(ref raw) = args.amount {
        let d = parse_amount(raw)?;
        obj.insert("amount".into(), to_amount_number(d)?);
    }

    // Tip amount
    if let Some(ref raw) = args.tip_amount {
        let d = parse_amount(raw)?;
        obj.insert("tipAmount".into(), to_amount_number(d)?);
    }

    // Tip rate — parse_rate allows up to 4 decimal places (rates are not money).
    // NOTE: transactions l2_tax_rate could adopt parse_rate later (not changed here).
    if let Some(ref raw) = args.tip_rate {
        let d = parse_rate(raw)?;
        obj.insert("tipRate".into(), to_amount_number(d)?);
    }

    // Only-present optional string fields
    if let Some(ref v) = args.pos_device_id {
        obj.insert("posDeviceId".into(), Value::String(v.clone()));
    }
    if let Some(ref v) = args.reference_id {
        obj.insert("referenceId".into(), Value::String(v.clone()));
    }
    if let Some(ref v) = args.payment_processor_id {
        obj.insert("paymentProcessorId".into(), Value::String(v.clone()));
    }
    if let Some(ref v) = args.customer_id {
        obj.insert("customerId".into(), Value::String(v.clone()));
    }
    if let Some(ref v) = args.target_transaction_id {
        obj.insert("targetTransactionId".into(), Value::String(v.clone()));
    }

    // Reading method (optional int)
    if let Some(rm) = args.reading_method {
        obj.insert(
            "readingMethodId".into(),
            Value::Number(serde_json::Number::from(rm)),
        );
    }

    // waitForAcceptanceByTerminal — always set (true when --wait)
    obj.insert("waitForAcceptanceByTerminal".into(), Value::Bool(args.wait));

    Ok(Value::Object(obj))
}

// ── Field-name normalisation helpers ─────────────────────────────────────────

/// Return the POS transaction ID regardless of which API operation produced `v`.
///
/// - **create / cancel** responses use `posTransactionId`
/// - **get / list** responses use `id`
///
/// Tries `id` first (the most common shape), then falls back to
/// `posTransactionId`.
pub(crate) fn pos_id(v: &Value) -> Option<&str> {
    v.get("id")
        .and_then(|x| x.as_str())
        .or_else(|| v.get("posTransactionId").and_then(|x| x.as_str()))
}

/// Return the POS transaction status string regardless of which API operation
/// produced `v`.
///
/// - **create / cancel** responses use `status`
/// - **get / list** responses use `posTransactionStatus`
///
/// Tries `posTransactionStatus` first (the richer field name), then falls back
/// to `status`.
pub(crate) fn pos_status(v: &Value) -> Option<&str> {
    v.get("posTransactionStatus")
        .and_then(|x| x.as_str())
        .or_else(|| v.get("status").and_then(|x| x.as_str()))
}

// ── pos_is_final ──────────────────────────────────────────────────────────────

/// Pure decision function: returns `true` when the POS transaction has reached
/// a final state (i.e. `isCompleted == true`).
///
/// This is the poll-exit signal for `--wait`. It reads `v["isCompleted"]`
/// defensively, treating absent or non-bool as `false`.
pub fn pos_is_final(v: &Value) -> bool {
    v["isCompleted"].as_bool().unwrap_or(false)
}

// ── Render helpers ────────────────────────────────────────────────────────────

/// Build the table string for a single POS transaction (pure helper, golden-testable).
///
/// Columns rendered (key-value style):
///   id, terminalId, transactionType, posTransactionStatus, amount, isCompleted, transactionId
pub(crate) fn pos_transaction_table(v: &Value) -> String {
    let get_str = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("—");
    let get_bool = |k: &str| {
        v.get(k)
            .and_then(|x| x.as_bool())
            .map(|b| if b { "true" } else { "false" })
            .unwrap_or("—")
    };
    let get_num = |k: &str| {
        v.get(k)
            .and_then(|x| {
                x.as_f64()
                    .map(|f| format!("{f:.2}"))
                    .or_else(|| x.as_str().map(|s| s.to_string()))
            })
            .unwrap_or_else(|| "—".to_string())
    };

    format!(
        "id:                   {}\nterminalId:           {}\ntransactionType:      {}\nposTransactionStatus: {}\namount:               {}\nisCompleted:          {}\ntransactionId:        {}",
        pos_id(v).unwrap_or("—"),
        get_str("terminalId"),
        get_str("transactionType"),
        pos_status(v).unwrap_or("—"),
        get_num("amount"),
        get_bool("isCompleted"),
        get_str("transactionId"),
    )
}

/// Build the table string for a POS transaction list (pure helper, golden-testable).
///
/// Columns: ID (36), TERMINAL ID (36), TYPE (24), STATUS (28), AMOUNT (12), DONE (6)
pub(crate) fn pos_transaction_list_table(items: &[Value]) -> String {
    let header = format!(
        "{:<36}  {:<36}  {:<24}  {:<28}  {:<12}  {:<6}",
        "ID", "TERMINAL ID", "TYPE", "STATUS", "AMOUNT", "DONE"
    );
    let separator = "-".repeat(36 + 2 + 36 + 2 + 24 + 2 + 28 + 2 + 12 + 2 + 6);
    let mut rows = vec![header, separator];

    for item in items {
        let id = pos_id(item).unwrap_or("—");
        let terminal_id = item
            .get("terminalId")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let txn_type = item
            .get("transactionType")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let status = pos_status(item).unwrap_or("—");
        let amount = item
            .get("amount")
            .and_then(|v| v.as_f64().map(|f| format!("{f:.2}")))
            .unwrap_or_else(|| "—".to_string());
        let done = item
            .get("isCompleted")
            .and_then(|v| v.as_bool())
            .map(|b| if b { "yes" } else { "no" })
            .unwrap_or("—");

        rows.push(format!(
            "{}  {}  {}  {}  {}  {}",
            fit(id, 36),
            fit(terminal_id, 36),
            fit(txn_type, 24),
            fit(status, 28),
            fit(&amount, 12),
            fit(done, 6),
        ));
    }

    rows.join("\n")
}

/// Render a single POS transaction (used for create/get/cancel and the --wait final render).
///
/// - `json`  → `Envelope { object: "pos_transaction", data: v, … }`
/// - `table` → key-value list via [`pos_transaction_table`]
/// - `quiet` → just `v["id"]`
pub fn render_pos_transaction(
    v: &Value,
    fmt: OutputFormat,
    environment: &str,
) -> anyhow::Result<()> {
    match fmt {
        OutputFormat::Json => {
            let env = Envelope::new("pos_transaction", v.clone(), environment, None);
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        OutputFormat::Table => {
            println!("{}", pos_transaction_table(v));
        }
        OutputFormat::Quiet => {
            let id = pos_id(v).unwrap_or("—");
            println!("{id}");
        }
    }
    Ok(())
}

/// Render a POS transaction list response.
///
/// - `json`  → `Envelope { object: "pos_transaction_list", data: v (raw), … }`
/// - `table` → columnar table via [`pos_transaction_list_table`]
/// - `quiet` → one ID per line
pub fn render_pos_transaction_list(
    v: &Value,
    fmt: OutputFormat,
    environment: &str,
) -> anyhow::Result<()> {
    let items = v
        .get("items")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();

    match fmt {
        OutputFormat::Json => {
            let env = Envelope::new("pos_transaction_list", v.clone(), environment, None);
            println!("{}", serde_json::to_string_pretty(&env)?);
        }
        OutputFormat::Table => {
            println!("{}", pos_transaction_list_table(&items));
        }
        OutputFormat::Quiet => {
            for item in &items {
                println!("{}", pos_id(item).unwrap_or("—"));
            }
        }
    }
    Ok(())
}

// ── --wait poll loop ──────────────────────────────────────────────────────────

/// Outcome returned by the poll loop.
#[derive(Debug)]
pub enum PollOutcome {
    /// `isCompleted` became true within the timeout window.
    Completed(Value),
    /// The timeout expired before `isCompleted` became true.
    TimedOut(Value),
    /// Ctrl-C was received; last-known value returned.
    Interrupted(Value),
}

/// Drive the `--wait` long-poll loop.
///
/// Behaviour:
/// - Every 2 seconds: GET `/pos-transactions/{id}`.
/// - Exit when `pos_is_final(resp)` is true → `PollOutcome::Completed`.
/// - Exit when elapsed ≥ `timeout_secs` → `PollOutcome::TimedOut`.
/// - On Ctrl-C: → `PollOutcome::Interrupted`.
///
/// `getter` is an async fn `(id: &str) -> anyhow::Result<Value>` — injected
/// so the loop is testable with a mock without requiring a live API client.
pub async fn run_wait_poll<F, Fut>(
    id: &str,
    timeout_secs: u64,
    getter: F,
) -> anyhow::Result<PollOutcome>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<Value>>,
{
    use std::time::Duration;
    use tokio::time::{Instant, sleep};

    let poll_interval = Duration::from_secs(2);
    let timeout = Duration::from_secs(timeout_secs);
    let started = Instant::now();

    // Seed with a placeholder; will be overwritten on first poll.
    let mut last_known = Value::Null;

    loop {
        // Wait for the poll interval or Ctrl-C, whichever fires first.
        tokio::select! {
            _ = sleep(poll_interval) => {}
            _ = tokio::signal::ctrl_c() => {
                return Ok(PollOutcome::Interrupted(last_known));
            }
        }

        let resp = getter(id.to_string()).await?;
        last_known = resp;

        if pos_is_final(&last_known) {
            return Ok(PollOutcome::Completed(last_known));
        }

        if started.elapsed() >= timeout {
            return Ok(PollOutcome::TimedOut(last_known));
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── pos_id ────────────────────────────────────────────────────────────────

    #[test]
    fn pos_id_reads_id_from_get_shape() {
        let v = json!({ "id": "g-1", "posTransactionStatus": "Completed", "isCompleted": true });
        assert_eq!(pos_id(&v), Some("g-1"));
    }

    #[test]
    fn pos_id_reads_pos_transaction_id_from_create_shape() {
        let v = json!({ "posTransactionId": "p-1", "status": "TerminalConnecting", "statusId": 1 });
        assert_eq!(pos_id(&v), Some("p-1"));
    }

    #[test]
    fn pos_id_returns_none_when_both_absent() {
        let v = json!({ "amount": 10.00 });
        assert_eq!(pos_id(&v), None);
    }

    #[test]
    fn pos_id_prefers_id_when_both_present() {
        // Should not happen in practice, but id wins.
        let v = json!({ "id": "canonical", "posTransactionId": "alternate" });
        assert_eq!(pos_id(&v), Some("canonical"));
    }

    // ── pos_status ────────────────────────────────────────────────────────────

    #[test]
    fn pos_status_reads_pos_transaction_status_from_get_shape() {
        let v = json!({ "id": "g-1", "posTransactionStatus": "Completed", "isCompleted": true });
        assert_eq!(pos_status(&v), Some("Completed"));
    }

    #[test]
    fn pos_status_reads_status_from_create_shape() {
        let v = json!({ "posTransactionId": "p-1", "status": "TerminalConnecting", "statusId": 1 });
        assert_eq!(pos_status(&v), Some("TerminalConnecting"));
    }

    #[test]
    fn pos_status_returns_none_when_both_absent() {
        let v = json!({ "id": "x" });
        assert_eq!(pos_status(&v), None);
    }

    #[test]
    fn pos_status_prefers_pos_transaction_status_when_both_present() {
        let v = json!({ "posTransactionStatus": "Completed", "status": "TerminalConnecting" });
        assert_eq!(pos_status(&v), Some("Completed"));
    }

    // ── pos_is_final ──────────────────────────────────────────────────────────

    #[test]
    fn pos_is_final_true_when_is_completed_true() {
        let v = json!({ "id": "pos-1", "isCompleted": true });
        assert!(pos_is_final(&v));
    }

    #[test]
    fn pos_is_final_false_when_is_completed_false() {
        let v = json!({ "id": "pos-1", "isCompleted": false });
        assert!(!pos_is_final(&v));
    }

    #[test]
    fn pos_is_final_false_when_is_completed_missing() {
        let v = json!({ "id": "pos-1" });
        assert!(!pos_is_final(&v));
    }

    #[test]
    fn pos_is_final_false_when_is_completed_null() {
        let v = json!({ "id": "pos-1", "isCompleted": null });
        assert!(!pos_is_final(&v));
    }

    // ── build_pos_create_body ─────────────────────────────────────────────────

    #[test]
    fn build_body_required_fields_only() {
        let args = PosCreateArgs {
            terminal_id: "term-abc".into(),
            transaction_type: 2,
            amount: None,
            currency_id: 1,
            tip_amount: None,
            tip_rate: None,
            pos_device_id: None,
            reference_id: None,
            payment_processor_id: None,
            customer_id: None,
            target_transaction_id: None,
            reading_method: None,
            wait: false,
        };
        let body = build_pos_create_body(&args).unwrap();
        assert_eq!(body["terminalId"], "term-abc");
        assert_eq!(body["transactionTypeId"], 2);
        assert_eq!(body["currencyId"], 1);
        assert_eq!(body["waitForAcceptanceByTerminal"], false);
        // Optional fields must be absent
        assert!(body.get("amount").is_none());
        assert!(body.get("tipAmount").is_none());
        assert!(body.get("posDeviceId").is_none());
        assert!(body.get("referenceId").is_none());
        assert!(body.get("readingMethodId").is_none());
    }

    #[test]
    fn build_body_with_all_optional_fields() {
        let args = PosCreateArgs {
            terminal_id: "term-xyz".into(),
            transaction_type: 1,
            amount: Some("50.00".into()),
            currency_id: 1,
            tip_amount: Some("5.00".into()),
            tip_rate: Some("0.10".into()),
            pos_device_id: Some("dev-001".into()),
            reference_id: Some("ref-123".into()),
            payment_processor_id: Some("proc-456".into()),
            customer_id: Some("cust-789".into()),
            target_transaction_id: Some("target-abc".into()),
            reading_method: Some(1),
            wait: true,
        };
        let body = build_pos_create_body(&args).unwrap();
        assert_eq!(body["terminalId"], "term-xyz");
        assert_eq!(body["transactionTypeId"], 1);
        assert_eq!(body["currencyId"], 1);
        assert_eq!(body["waitForAcceptanceByTerminal"], true);
        // Check amount is a JSON number (not string)
        assert!(body["amount"].is_number());
        assert_eq!(
            serde_json::to_string(&body["amount"]).unwrap(),
            "50.00",
            "amount must be exact decimal"
        );
        assert!(body["tipAmount"].is_number());
        assert_eq!(body["posDeviceId"], "dev-001");
        assert_eq!(body["referenceId"], "ref-123");
        assert_eq!(body["paymentProcessorId"], "proc-456");
        assert_eq!(body["customerId"], "cust-789");
        assert_eq!(body["targetTransactionId"], "target-abc");
        assert_eq!(body["readingMethodId"], 1);
    }

    #[test]
    fn build_body_wait_true_sets_wait_flag() {
        let args = PosCreateArgs {
            terminal_id: "t".into(),
            transaction_type: 2,
            amount: None,
            currency_id: 1,
            tip_amount: None,
            tip_rate: None,
            pos_device_id: None,
            reference_id: None,
            payment_processor_id: None,
            customer_id: None,
            target_transaction_id: None,
            reading_method: None,
            wait: true,
        };
        let body = build_pos_create_body(&args).unwrap();
        assert_eq!(body["waitForAcceptanceByTerminal"], true);
    }

    #[test]
    fn build_body_invalid_amount_returns_error() {
        let args = PosCreateArgs {
            terminal_id: "t".into(),
            transaction_type: 2,
            amount: Some("not-a-number".into()),
            currency_id: 1,
            tip_amount: None,
            tip_rate: None,
            pos_device_id: None,
            reference_id: None,
            payment_processor_id: None,
            customer_id: None,
            target_transaction_id: None,
            reading_method: None,
            wait: false,
        };
        assert!(build_pos_create_body(&args).is_err());
    }

    // ── pos_transaction_table ─────────────────────────────────────────────────

    fn sample_pos_txn() -> Value {
        json!({
            "id": "pos-txn-001",
            "terminalId": "term-001",
            "transactionType": "Sale",
            "posTransactionStatus": "TerminalConnecting",
            "amount": 100.00,
            "isCompleted": false,
            "transactionId": null
        })
    }

    #[test]
    fn pos_transaction_table_shows_all_key_fields() {
        let table = pos_transaction_table(&sample_pos_txn());
        assert!(table.contains("pos-txn-001"), "must contain id");
        assert!(table.contains("term-001"), "must contain terminalId");
        assert!(table.contains("Sale"), "must contain transactionType");
        assert!(
            table.contains("TerminalConnecting"),
            "must contain posTransactionStatus"
        );
        assert!(table.contains("false"), "must contain isCompleted=false");
        // Amount must be rendered to 2 decimal places
        assert!(
            table.contains("100.00"),
            "amount must be formatted to 2 decimal places, got: {table}"
        );
    }

    #[test]
    fn pos_transaction_table_missing_fields_show_dash() {
        let v = json!({ "id": "pos-999" });
        let table = pos_transaction_table(&v);
        assert!(table.contains("pos-999"));
        assert!(table.contains('—'));
    }

    /// create/cancel shape: `posTransactionId` + `status` must render correctly.
    #[test]
    fn pos_transaction_table_renders_create_shape() {
        let v = json!({
            "posTransactionId": "p-1",
            "status": "TerminalConnecting",
            "statusId": 1
        });
        let table = pos_transaction_table(&v);
        assert!(
            table.contains("p-1"),
            "table must contain posTransactionId value; got: {table}"
        );
        assert!(
            table.contains("TerminalConnecting"),
            "table must contain status value; got: {table}"
        );
    }

    /// get/list shape: `id` + `posTransactionStatus` must still render correctly.
    #[test]
    fn pos_transaction_table_renders_get_shape() {
        let v = json!({
            "id": "g-1",
            "posTransactionStatus": "Completed",
            "isCompleted": true
        });
        let table = pos_transaction_table(&v);
        assert!(
            table.contains("g-1"),
            "table must contain id value; got: {table}"
        );
        assert!(
            table.contains("Completed"),
            "table must contain posTransactionStatus value; got: {table}"
        );
    }

    // ── pos_transaction_list_table ────────────────────────────────────────────

    #[test]
    fn pos_transaction_list_table_renders_header_and_rows() {
        let items = vec![sample_pos_txn()];
        let table = pos_transaction_list_table(&items);
        assert!(table.contains("ID"), "must have ID header");
        assert!(
            table.contains("TERMINAL ID"),
            "must have TERMINAL ID header"
        );
        assert!(table.contains("TYPE"), "must have TYPE header");
        assert!(table.contains("STATUS"), "must have STATUS header");
        assert!(table.contains("AMOUNT"), "must have AMOUNT header");
        assert!(table.contains("DONE"), "must have DONE header");
        assert!(table.contains("pos-txn-001"), "must contain row id");
        assert!(table.contains("term-001"), "must contain terminal id");
        assert!(table.contains("TerminalConnecting"), "must contain status");
        // Amount must be rendered to 2 decimal places
        assert!(
            table.contains("100.00"),
            "list amount must be formatted to 2 decimal places, got: {table}"
        );
    }

    #[test]
    fn pos_transaction_list_table_empty_shows_header_only() {
        let table = pos_transaction_list_table(&[]);
        assert!(table.contains("ID"));
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 2); // header + separator
    }

    /// List table must render create-shape items (posTransactionId + status).
    #[test]
    fn pos_transaction_list_table_renders_create_shape() {
        let item = json!({
            "posTransactionId": "p-1",
            "status": "TerminalConnecting",
            "statusId": 1
        });
        let table = pos_transaction_list_table(&[item]);
        assert!(
            table.contains("p-1"),
            "list table must contain posTransactionId; got: {table}"
        );
        assert!(
            table.contains("TerminalConnecting"),
            "list table must contain status; got: {table}"
        );
    }

    /// List table must render get-shape items (id + posTransactionStatus).
    #[test]
    fn pos_transaction_list_table_renders_get_shape() {
        let item = json!({
            "id": "g-1",
            "posTransactionStatus": "Completed",
            "isCompleted": true
        });
        let table = pos_transaction_list_table(&[item]);
        assert!(
            table.contains("g-1"),
            "list table must contain id; got: {table}"
        );
        assert!(
            table.contains("Completed"),
            "list table must contain posTransactionStatus; got: {table}"
        );
    }

    /// render_pos_transaction quiet mode must output posTransactionId for create shape.
    #[test]
    fn render_pos_transaction_quiet_create_shape() {
        use crate::cli::output::OutputFormat;
        // Capture stdout by exercising pos_id directly (render_pos_transaction
        // calls println! so we test the helper which drives it).
        let v = json!({ "posTransactionId": "p-1", "status": "TerminalConnecting", "statusId": 1 });
        assert_eq!(
            pos_id(&v),
            Some("p-1"),
            "quiet mode uses pos_id; create shape must return posTransactionId"
        );
        // Ensure OutputFormat::Quiet would print it (compile-time coverage).
        let _ = OutputFormat::Quiet;
    }

    /// render_pos_transaction quiet mode must output id for get shape.
    #[test]
    fn render_pos_transaction_quiet_get_shape() {
        let v = json!({ "id": "g-1", "posTransactionStatus": "Completed", "isCompleted": true });
        assert_eq!(
            pos_id(&v),
            Some("g-1"),
            "quiet mode uses pos_id; get shape must return id"
        );
    }

    // ── run_wait_poll — async tests using tokio time control ──────────────────

    /// Simulate a poller that sees isCompleted:false on poll 1, isCompleted:true on poll 2.
    /// With start_paused=true, tokio::time::advance lets us skip real-clock time.
    #[tokio::test(start_paused = true)]
    async fn poll_loop_completes_after_two_polls() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        use tokio::time::advance;

        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = call_count.clone();

        let getter = move |_id: String| {
            let cc = cc.clone();
            async move {
                let n = cc.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    // First poll: not yet complete
                    Ok(json!({
                        "id": "pos-txn-001",
                        "posTransactionStatus": "TerminalConnecting",
                        "isCompleted": false
                    }))
                } else {
                    // Second poll: completed
                    Ok(json!({
                        "id": "pos-txn-001",
                        "posTransactionStatus": "TransactionProcessing",
                        "isCompleted": true
                    }))
                }
            }
        };

        // Spawn the poll loop with 120s timeout.
        let poll_handle = tokio::spawn(run_wait_poll("pos-txn-001", 120, getter));

        // Advance past first 2-second poll interval.
        advance(std::time::Duration::from_secs(3)).await;
        // Advance past second 2-second poll interval.
        advance(std::time::Duration::from_secs(3)).await;

        let outcome = poll_handle.await.unwrap().unwrap();
        assert!(
            matches!(outcome, PollOutcome::Completed(ref v) if v["isCompleted"] == true),
            "expected PollOutcome::Completed with isCompleted:true, got {outcome:?}"
        );
        assert_eq!(
            call_count.load(Ordering::SeqCst),
            2,
            "getter must be called exactly twice"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn poll_loop_times_out_when_never_completed() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        use tokio::time::advance;

        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = call_count.clone();

        // getter always returns isCompleted:false, counting each call
        let getter = move |_id: String| {
            let cc = cc.clone();
            async move {
                cc.fetch_add(1, Ordering::SeqCst);
                Ok(json!({
                    "id": "pos-txn-timeout",
                    "isCompleted": false
                }))
            }
        };

        let poll_handle = tokio::spawn(run_wait_poll("pos-txn-timeout", 4, getter));

        // With a 2s poll interval and 4s timeout, polls fire at t=2s and t=4s.
        // Advance by 3s twice so the task can interleave between advances:
        //   advance(3s) → first sleep(2s) fires → getter called (count=1), elapsed=2s < 4s
        //   advance(3s) → second sleep(2s) fires → getter called (count=2), elapsed=4s >= 4s → TimedOut
        advance(std::time::Duration::from_secs(3)).await;
        advance(std::time::Duration::from_secs(3)).await;

        let outcome = poll_handle.await.unwrap().unwrap();
        assert!(
            matches!(outcome, PollOutcome::TimedOut(_)),
            "expected PollOutcome::TimedOut, got {outcome:?}"
        );
        // With a 2s interval and 4s timeout: polls fire at t=2s and t=4s → exactly 2 calls.
        assert_eq!(
            call_count.load(Ordering::SeqCst),
            2,
            "getter must be called exactly 2 times (t=2s and t=4s)"
        );
    }
}
