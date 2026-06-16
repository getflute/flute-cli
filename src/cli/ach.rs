//! CLI handlers for the ACH command group (`flute ach …`).
//!
//! The body builder mirrors `build_sale_body` in the transactions module.
//! ACH debit and credit share the same request shape (CreateIsvAchPaymentRequestDto /
//! CreateIsvAchRefundRequestDto) and therefore share a single `build_ach_body` builder.

use anyhow::Result;
use clap::ValueEnum;
use rust_decimal::Decimal;
use serde_json::{Map, Value, json};

use crate::api::models::to_amount_number;

// ── Enum args ─────────────────────────────────────────────────────────────────

/// Maps the `--account-type` CLI token to the API integer enum.
///
/// - `checking` → 1 (Checking / AccountTypeDto::Checking)
/// - `savings`  → 2 (Savings  / AccountTypeDto::Savings)
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AccountTypeArg {
    Checking,
    Savings,
}

impl AccountTypeArg {
    /// Returns the integer value expected by the API wire format.
    pub(crate) fn to_api_int(self) -> i32 {
        match self {
            AccountTypeArg::Checking => 1,
            AccountTypeArg::Savings => 2,
        }
    }
}

/// Maps the `--account-holder-type` CLI token to the API integer enum.
///
/// - `business`  → 1 (Business / AccountHolderTypeDto::Business)
/// - `personal`  → 2 (Personal / AccountHolderTypeDto::Personal)
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AccountHolderTypeArg {
    Business,
    Personal,
}

impl AccountHolderTypeArg {
    /// Returns the integer value expected by the API wire format.
    pub(crate) fn to_api_int(self) -> i32 {
        match self {
            AccountHolderTypeArg::Business => 1,
            AccountHolderTypeArg::Personal => 2,
        }
    }
}

// ── Body-builder ─────────────────────────────────────────────────────────────

/// All CLI flags that feed an ACH `debit` or `credit` request.
///
/// Debit (`POST /pay-api/v1/transactions/ach/payment`) and credit
/// (`POST /pay-api/v1/transactions/ach/payment/credit`) share the identical
/// request body shape, so a single args struct + builder covers both.
pub struct AchArgs {
    /// Transaction amount (required).
    pub amount: Decimal,
    /// `paymentProcessorId` — required, no default.
    pub payment_processor_id: String,
    /// `requesterIpAddress` — default `"127.0.0.1"`.
    pub requester_ip: String,
    /// `secCode` — AchSECCodeDto int; default `1` (Web).
    pub sec_code: i32,
    /// `routingNumber` — optional.
    pub routing: Option<String>,
    /// `accountNumber` — optional.
    pub account: Option<String>,
    /// `accountType` — maps to 1=Checking, 2=Savings; optional (omit sends no field).
    pub account_type: Option<AccountTypeArg>,
    /// `accountHolderType` — maps to 1=Business, 2=Personal; live-required for debit/credit.
    pub account_holder_type: Option<AccountHolderTypeArg>,
    /// `taxId` — optional.
    pub tax_id: Option<String>,
    /// `customerId` — optional.
    pub customer_id: Option<String>,
    /// `paymentMethodId` — optional.
    pub payment_method_id: Option<String>,
    /// `isFasterProcessing` — default `false`; always included (non-nullable API field).
    pub faster: bool,

    // ── billingAddress (AddressIsvDto) ────────────────────────────────────────
    /// `billingAddress.line1` — optional.
    pub billing_line1: Option<String>,
    /// `billingAddress.line2` — optional.
    pub billing_line2: Option<String>,
    /// `billingAddress.city` — optional.
    pub billing_city: Option<String>,
    /// `billingAddress.stateName` — optional (free-text state name).
    pub billing_state: Option<String>,
    /// `billingAddress.stateId` — numeric state ID required by live API (US: use integer ID; countryId must be 1).
    pub billing_state_id: Option<i32>,
    /// `billingAddress.postalCode` — optional.
    pub billing_postal_code: Option<String>,
    /// `billingAddress.countryId` — optional (US = 1).
    pub billing_country_id: Option<i32>,

    // ── contactInfo (ContactInfoIsvDto) ───────────────────────────────────────
    /// `contactInfo.firstName` — optional.
    pub contact_first_name: Option<String>,
    /// `contactInfo.lastName` — optional.
    pub contact_last_name: Option<String>,
    /// `contactInfo.email` — optional.
    pub contact_email: Option<String>,
    /// `contactInfo.mobileNumber` — optional.
    pub contact_phone: Option<String>,
    /// `contactInfo.companyName` — optional.
    pub contact_company: Option<String>,
}

/// Build the JSON request body for an ACH `debit` or `credit` request.
///
/// This is a **pure function** — no I/O, no network, trivially unit-testable.
///
/// ## Field mapping
/// | Arg field             | Wire key               | Notes                                   |
/// |-----------------------|------------------------|-----------------------------------------|
/// | `amount`              | `amount`               | exact decimal via `to_amount_number`    |
/// | `payment_processor_id`| `paymentProcessorId`   | required, always present                |
/// | `requester_ip`        | `requesterIpAddress`   | required, default `"127.0.0.1"`         |
/// | `sec_code`            | `secCode`              | required, default `1` (Web)             |
/// | `routing`             | `routingNumber`        | optional, omitted when `None`           |
/// | `account`             | `accountNumber`        | optional, omitted when `None`           |
/// | `account_type`        | `accountType`          | int 1/2; optional, omitted when `None`  |
/// | `account_holder_type` | `accountHolderType`    | int 1/2; live-required; omitted when `None` |
/// | `tax_id`              | `taxId`                | optional, omitted when `None`           |
/// | `customer_id`         | `customerId`           | optional, omitted when `None`           |
/// | `payment_method_id`   | `paymentMethodId`      | optional, omitted when `None`           |
/// | `faster`              | `isFasterProcessing`   | always present (non-nullable), default false |
/// | `billing_*`           | `billingAddress{…}`    | AddressIsvDto; included only when ≥1 field present; `billing_state_id`→`stateId` (int, live-required for US; use with `countryId=1`) |
/// | `contact_*`           | `contactInfo{…}`       | ContactInfoIsvDto; included only when ≥1 field present |
pub fn build_ach_body(args: &AchArgs) -> Result<Value> {
    let mut obj = Map::new();

    // Required fields — always present
    obj.insert("amount".into(), to_amount_number(args.amount)?);
    obj.insert(
        "paymentProcessorId".into(),
        Value::String(args.payment_processor_id.clone()),
    );
    obj.insert(
        "requesterIpAddress".into(),
        Value::String(args.requester_ip.clone()),
    );
    obj.insert("secCode".into(), json!(args.sec_code));

    // Optional ACH account fields — omit when None
    if let Some(routing) = &args.routing {
        obj.insert("routingNumber".into(), Value::String(routing.clone()));
    }
    if let Some(account) = &args.account {
        obj.insert("accountNumber".into(), Value::String(account.clone()));
    }
    if let Some(account_type) = args.account_type {
        obj.insert("accountType".into(), json!(account_type.to_api_int()));
    }
    if let Some(holder_type) = args.account_holder_type {
        obj.insert("accountHolderType".into(), json!(holder_type.to_api_int()));
    }
    if let Some(tax_id) = &args.tax_id {
        obj.insert("taxId".into(), Value::String(tax_id.clone()));
    }

    // Optional vault references — omit when None
    if let Some(id) = &args.customer_id {
        obj.insert("customerId".into(), Value::String(id.clone()));
    }
    if let Some(id) = &args.payment_method_id {
        obj.insert("paymentMethodId".into(), Value::String(id.clone()));
    }

    // isFasterProcessing — always present (non-nullable field); default false
    obj.insert("isFasterProcessing".into(), json!(args.faster));

    // billingAddress — include only when at least one field is present
    {
        let mut addr = Map::new();
        if let Some(v) = &args.billing_line1 {
            addr.insert("line1".into(), Value::String(v.clone()));
        }
        if let Some(v) = &args.billing_line2 {
            addr.insert("line2".into(), Value::String(v.clone()));
        }
        if let Some(v) = &args.billing_city {
            addr.insert("city".into(), Value::String(v.clone()));
        }
        if let Some(v) = &args.billing_state {
            addr.insert("stateName".into(), Value::String(v.clone()));
        }
        if let Some(v) = args.billing_state_id {
            addr.insert("stateId".into(), json!(v));
        }
        if let Some(v) = &args.billing_postal_code {
            addr.insert("postalCode".into(), Value::String(v.clone()));
        }
        if let Some(v) = args.billing_country_id {
            addr.insert("countryId".into(), json!(v));
        }
        if !addr.is_empty() {
            obj.insert("billingAddress".into(), Value::Object(addr));
        }
    }

    // contactInfo — include only when at least one field is present
    {
        let mut contact = Map::new();
        if let Some(v) = &args.contact_first_name {
            contact.insert("firstName".into(), Value::String(v.clone()));
        }
        if let Some(v) = &args.contact_last_name {
            contact.insert("lastName".into(), Value::String(v.clone()));
        }
        if let Some(v) = &args.contact_email {
            contact.insert("email".into(), Value::String(v.clone()));
        }
        if let Some(v) = &args.contact_phone {
            contact.insert("mobileNumber".into(), Value::String(v.clone()));
        }
        if let Some(v) = &args.contact_company {
            contact.insert("companyName".into(), Value::String(v.clone()));
        }
        if !contact.is_empty() {
            obj.insert("contactInfo".into(), Value::Object(contact));
        }
    }

    Ok(Value::Object(obj))
}

// ── ACH transaction dispatcher ───────────────────────────────────────────────

/// Selects which ACH transaction endpoint to call.
pub(crate) enum AchTxnKind {
    Debit,
    Credit,
}

/// Shared handler for ACH debit and credit, which share the same request body
/// shape and differ only in which API endpoint they call.
pub(crate) async fn execute_ach_txn(
    profile: &str,
    output: crate::cli::output::OutputFormat,
    args: AchArgs,
    kind: AchTxnKind,
) -> anyhow::Result<()> {
    let body = build_ach_body(&args)?;
    let (p, api) = crate::build_client(profile)?;
    let result = match kind {
        AchTxnKind::Debit => api.ach_debit(body).await?,
        AchTxnKind::Credit => api.ach_credit(body).await?,
    };
    crate::cli::transactions::render_transaction(&result, output, &p.name)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn base_args() -> AchArgs {
        AchArgs {
            amount: Decimal::from_str("500.00").unwrap(),
            payment_processor_id: "pp-1".into(),
            requester_ip: "127.0.0.1".into(),
            sec_code: 1,
            routing: Some("021000021".into()),
            account: Some("123456789".into()),
            account_type: Some(AccountTypeArg::Checking),
            account_holder_type: None,
            tax_id: None,
            customer_id: None,
            payment_method_id: None,
            faster: false,
            billing_line1: None,
            billing_line2: None,
            billing_city: None,
            billing_state: None,
            billing_state_id: None,
            billing_postal_code: None,
            billing_country_id: None,
            contact_first_name: None,
            contact_last_name: None,
            contact_email: None,
            contact_phone: None,
            contact_company: None,
        }
    }

    // ── Golden test: build_ach_debit_body_maps_flags ──────────────────────────

    /// Golden test: an `AchArgs` (debit) maps to the exact expected JSON fields.
    ///
    /// Asserts:
    /// - `amount`           = JSON number `500.00` (not a string, not a float artifact)
    /// - `routingNumber`    = "021000021"
    /// - `accountNumber`    = "123456789"
    /// - `accountType`      = 1 (Checking)
    /// - `paymentProcessorId` = "pp-1"
    /// - `requesterIpAddress` = "127.0.0.1"
    /// - `secCode`          = 1
    /// - `isFasterProcessing` = false
    #[test]
    fn build_ach_debit_body_maps_flags() {
        let args = base_args();
        let body = build_ach_body(&args).unwrap();

        // amount must be a JSON number exactly "500.00"
        assert!(
            body["amount"].is_number(),
            "amount must be a JSON number, got {:?}",
            body["amount"]
        );
        assert_eq!(
            serde_json::to_string(&body["amount"]).unwrap(),
            "500.00",
            "amount must serialise as 500.00 (exact decimal)"
        );

        // ACH account fields
        assert_eq!(body["routingNumber"], "021000021");
        assert_eq!(body["accountNumber"], "123456789");
        assert_eq!(body["accountType"], 1, "Checking maps to int 1");

        // Required infra fields
        assert_eq!(body["paymentProcessorId"], "pp-1");
        assert_eq!(body["requesterIpAddress"], "127.0.0.1");
        assert_eq!(body["secCode"], 1);

        // isFasterProcessing always present
        assert_eq!(body["isFasterProcessing"], false);

        // accountHolderType must be absent (not passed)
        assert!(
            body.get("accountHolderType").is_none(),
            "accountHolderType must be absent when not passed"
        );
    }

    /// Golden test: same builder used for credit — confirms reuse.
    ///
    /// The credit path just calls a different API endpoint; the body is identical.
    #[test]
    fn build_ach_credit_body_reuses_same_builder() {
        // Credit uses a savings account with a holder type
        let args = AchArgs {
            amount: Decimal::from_str("150.00").unwrap(),
            payment_processor_id: "pp-credit-1".into(),
            requester_ip: "127.0.0.1".into(),
            sec_code: 1,
            routing: Some("011000138".into()),
            account: Some("987654321".into()),
            account_type: Some(AccountTypeArg::Savings),
            account_holder_type: Some(AccountHolderTypeArg::Business),
            tax_id: None,
            customer_id: None,
            payment_method_id: None,
            faster: false,
            billing_line1: None,
            billing_line2: None,
            billing_city: None,
            billing_state: None,
            billing_state_id: None,
            billing_postal_code: None,
            billing_country_id: None,
            contact_first_name: None,
            contact_last_name: None,
            contact_email: None,
            contact_phone: None,
            contact_company: None,
        };

        let body = build_ach_body(&args).unwrap();

        assert_eq!(
            serde_json::to_string(&body["amount"]).unwrap(),
            "150.00",
            "amount must serialise as 150.00"
        );
        assert_eq!(body["accountType"], 2, "Savings maps to int 2");
        assert_eq!(body["accountHolderType"], 1, "Business maps to int 1");
        assert_eq!(body["paymentProcessorId"], "pp-credit-1");
        assert_eq!(body["secCode"], 1);
        assert_eq!(body["isFasterProcessing"], false);
    }

    // ── Additional unit tests ────────────────────────────────────────────────

    #[test]
    fn build_ach_body_omits_optional_fields_when_absent() {
        let args = AchArgs {
            amount: Decimal::from_str("10.00").unwrap(),
            payment_processor_id: "pp-min".into(),
            requester_ip: "127.0.0.1".into(),
            sec_code: 1,
            routing: None,
            account: None,
            account_type: None,
            account_holder_type: None,
            tax_id: None,
            customer_id: None,
            payment_method_id: None,
            faster: false,
            billing_line1: None,
            billing_line2: None,
            billing_city: None,
            billing_state: None,
            billing_state_id: None,
            billing_postal_code: None,
            billing_country_id: None,
            contact_first_name: None,
            contact_last_name: None,
            contact_email: None,
            contact_phone: None,
            contact_company: None,
        };

        let body = build_ach_body(&args).unwrap();

        // Required fields always present
        assert!(body["amount"].is_number());
        assert_eq!(body["paymentProcessorId"], "pp-min");
        assert_eq!(body["requesterIpAddress"], "127.0.0.1");
        assert_eq!(body["secCode"], 1);
        assert_eq!(body["isFasterProcessing"], false);

        // All optional fields absent
        assert!(body.get("routingNumber").is_none());
        assert!(body.get("accountNumber").is_none());
        assert!(body.get("accountType").is_none());
        assert!(body.get("accountHolderType").is_none());
        assert!(body.get("taxId").is_none());
        assert!(body.get("customerId").is_none());
        assert!(body.get("paymentMethodId").is_none());
    }

    #[test]
    fn build_ach_body_faster_processing_true() {
        let args = AchArgs {
            amount: Decimal::from_str("75.50").unwrap(),
            payment_processor_id: "pp-fast".into(),
            requester_ip: "10.0.0.1".into(),
            sec_code: 2,
            routing: None,
            account: None,
            account_type: None,
            account_holder_type: None,
            tax_id: None,
            customer_id: None,
            payment_method_id: None,
            faster: true,
            billing_line1: None,
            billing_line2: None,
            billing_city: None,
            billing_state: None,
            billing_state_id: None,
            billing_postal_code: None,
            billing_country_id: None,
            contact_first_name: None,
            contact_last_name: None,
            contact_email: None,
            contact_phone: None,
            contact_company: None,
        };

        let body = build_ach_body(&args).unwrap();
        assert_eq!(body["isFasterProcessing"], true);
        assert_eq!(body["secCode"], 2);
        assert_eq!(body["requesterIpAddress"], "10.0.0.1");
    }

    #[test]
    fn build_ach_body_includes_vault_fields_when_present() {
        let args = AchArgs {
            amount: Decimal::from_str("200.00").unwrap(),
            payment_processor_id: "pp-vault".into(),
            requester_ip: "127.0.0.1".into(),
            sec_code: 1,
            routing: None,
            account: None,
            account_type: None,
            account_holder_type: None,
            tax_id: Some("123-45-6789".into()),
            customer_id: Some("cust-uuid-001".into()),
            payment_method_id: Some("pm-uuid-001".into()),
            faster: false,
            billing_line1: None,
            billing_line2: None,
            billing_city: None,
            billing_state: None,
            billing_state_id: None,
            billing_postal_code: None,
            billing_country_id: None,
            contact_first_name: None,
            contact_last_name: None,
            contact_email: None,
            contact_phone: None,
            contact_company: None,
        };

        let body = build_ach_body(&args).unwrap();
        assert_eq!(body["taxId"], "123-45-6789");
        assert_eq!(body["customerId"], "cust-uuid-001");
        assert_eq!(body["paymentMethodId"], "pm-uuid-001");
    }

    // ── billing + contact golden tests ───────────────────────────────────────

    /// Golden test: billing address fields map to the correct camelCase wire keys,
    /// including the numeric `stateId` required by the live API.
    #[test]
    fn build_ach_debit_body_with_billing_and_contact() {
        let mut args = base_args();
        args.account_holder_type = Some(AccountHolderTypeArg::Business);
        args.billing_line1 = Some("123 Main St".into());
        args.billing_line2 = Some("Suite 4".into());
        args.billing_city = Some("Springfield".into());
        args.billing_state = Some("IL".into());
        args.billing_state_id = Some(14); // IL numeric state ID; live API requires stateId (int)
        args.billing_postal_code = Some("62701".into());
        args.billing_country_id = Some(1); // US = 1 (live-required alongside stateId)
        args.contact_first_name = Some("Jane".into());
        args.contact_last_name = Some("Doe".into());
        args.contact_email = Some("jane@example.com".into());
        args.contact_phone = Some("5550001234".into());
        args.contact_company = Some("Acme Corp".into());

        let body = build_ach_body(&args).unwrap();

        // accountHolderType must be present (live-required)
        assert_eq!(body["accountHolderType"], 1, "Business maps to int 1");

        // billingAddress nested object — exact camelCase wire keys
        let billing = &body["billingAddress"];
        assert_eq!(billing["line1"], "123 Main St");
        assert_eq!(billing["line2"], "Suite 4");
        assert_eq!(billing["city"], "Springfield");
        assert_eq!(billing["stateName"], "IL");
        // stateId must be emitted as an integer (live API rejects free-text)
        assert_eq!(billing["stateId"], 14, "stateId must be present as integer");
        assert_eq!(billing["postalCode"], "62701");
        assert_eq!(billing["countryId"], 1, "US countryId must be 1");

        // contactInfo nested object — exact camelCase wire keys
        let contact = &body["contactInfo"];
        assert_eq!(contact["firstName"], "Jane");
        assert_eq!(contact["lastName"], "Doe");
        assert_eq!(contact["email"], "jane@example.com");
        assert_eq!(contact["mobileNumber"], "5550001234");
        assert_eq!(contact["companyName"], "Acme Corp");

        // Existing required fields still intact
        assert_eq!(body["routingNumber"], "021000021");
        assert_eq!(body["accountNumber"], "123456789");
        assert_eq!(body["accountType"], 1);
        assert_eq!(body["paymentProcessorId"], "pp-1");
        assert_eq!(body["isFasterProcessing"], false);
    }

    /// Golden test: stateId alone (no stateName) is accepted — partial billing works.
    #[test]
    fn build_ach_body_billing_state_id_only() {
        let mut args = base_args();
        args.billing_state_id = Some(14);
        args.billing_country_id = Some(1);
        let body = build_ach_body(&args).unwrap();
        let billing = &body["billingAddress"];
        assert_eq!(
            billing["stateId"], 14,
            "stateId must be present as integer when provided"
        );
        assert_eq!(billing["countryId"], 1);
        assert!(
            billing.get("stateName").is_none(),
            "stateName must be absent when not provided"
        );
    }

    /// billingAddress is omitted entirely when no billing fields are provided.
    #[test]
    fn build_ach_body_omits_billing_when_all_billing_fields_absent() {
        let args = base_args();
        let body = build_ach_body(&args).unwrap();
        assert!(
            body.get("billingAddress").is_none(),
            "billingAddress must be absent when no billing fields given"
        );
    }

    /// contactInfo is omitted entirely when no contact fields are provided.
    #[test]
    fn build_ach_body_omits_contact_when_all_contact_fields_absent() {
        let args = base_args();
        let body = build_ach_body(&args).unwrap();
        assert!(
            body.get("contactInfo").is_none(),
            "contactInfo must be absent when no contact fields given"
        );
    }

    /// billingAddress with only line1 — partial fields work; line2/city/etc absent from sub-object.
    #[test]
    fn build_ach_body_billing_partial_fields() {
        let mut args = base_args();
        args.billing_line1 = Some("456 Oak Ave".into());
        let body = build_ach_body(&args).unwrap();
        let billing = &body["billingAddress"];
        assert_eq!(billing["line1"], "456 Oak Ave");
        assert!(billing.get("line2").is_none());
        assert!(billing.get("city").is_none());
        assert!(billing.get("countryId").is_none());
    }

    /// contactInfo with only email — partial fields work.
    #[test]
    fn build_ach_body_contact_partial_fields() {
        let mut args = base_args();
        args.contact_email = Some("contact@example.com".into());
        let body = build_ach_body(&args).unwrap();
        let contact = &body["contactInfo"];
        assert_eq!(contact["email"], "contact@example.com");
        assert!(contact.get("firstName").is_none());
        assert!(contact.get("lastName").is_none());
        assert!(contact.get("mobileNumber").is_none());
        assert!(contact.get("companyName").is_none());
    }

    #[test]
    fn account_type_arg_to_api_int() {
        assert_eq!(AccountTypeArg::Checking.to_api_int(), 1);
        assert_eq!(AccountTypeArg::Savings.to_api_int(), 2);
    }

    #[test]
    fn account_holder_type_arg_to_api_int() {
        assert_eq!(AccountHolderTypeArg::Business.to_api_int(), 1);
        assert_eq!(AccountHolderTypeArg::Personal.to_api_int(), 2);
    }

    #[test]
    fn build_ach_body_personal_holder_type_maps_to_2() {
        let args = AchArgs {
            amount: Decimal::from_str("50.00").unwrap(),
            payment_processor_id: "pp-personal".into(),
            requester_ip: "127.0.0.1".into(),
            sec_code: 1,
            routing: None,
            account: None,
            account_type: None,
            account_holder_type: Some(AccountHolderTypeArg::Personal),
            tax_id: None,
            customer_id: None,
            payment_method_id: None,
            faster: false,
            billing_line1: None,
            billing_line2: None,
            billing_city: None,
            billing_state: None,
            billing_state_id: None,
            billing_postal_code: None,
            billing_country_id: None,
            contact_first_name: None,
            contact_last_name: None,
            contact_email: None,
            contact_phone: None,
            contact_company: None,
        };

        let body = build_ach_body(&args).unwrap();
        assert_eq!(body["accountHolderType"], 2, "Personal maps to int 2");
    }
}
