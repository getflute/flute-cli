//! Shared billing-address input for AVS-sensitive flows (ARISE-4706).
//!
//! The same `--billing-*` CLI vocabulary feeds two endpoints that spell the
//! fields differently on the wire, so this module holds the uniform CLI struct
//! plus one mapper per endpoint:
//!
//! | CLI flag            | transactions key | customers key   |
//! |---------------------|------------------|-----------------|
//! | `--billing-line1`   | `line1`          | `addressLine1`  |
//! | `--billing-line2`   | `line2`          | `addressLine2`  |
//! | `--billing-city`    | `city`           | `city`          |
//! | `--billing-postal-code` | `postalCode` | `zip`           |
//! | `--billing-state`   | `stateName`      | `stateName`     |
//! | `--billing-state-id`| `stateId`        | `stateId`       |
//! | `--billing-country-id` | `countryId`   | `countryId`     |

use serde_json::{Map, Value, json};

/// The uniform set of billing-address inputs collected from `--billing-*` flags.
#[derive(Debug, Default, Clone)]
pub(crate) struct BillingArgs {
    pub line1: Option<String>,
    pub line2: Option<String>,
    pub city: Option<String>,
    pub state_name: Option<String>,
    pub state_id: Option<i32>,
    pub postal_code: Option<String>,
    pub country_id: Option<i32>,
}

impl BillingArgs {
    /// True when the caller supplied at least one billing field.
    pub fn is_present(&self) -> bool {
        self.line1.is_some()
            || self.line2.is_some()
            || self.city.is_some()
            || self.state_name.is_some()
            || self.state_id.is_some()
            || self.postal_code.is_some()
            || self.country_id.is_some()
    }
}

/// Build the **transaction** `billingAddress` object, or `None` if no field was
/// supplied. Keys: `line1`/`line2`/`city`/`stateName`/`stateId`/`postalCode`/`countryId`.
pub(crate) fn billing_transaction_json(b: &BillingArgs) -> Option<Value> {
    if !b.is_present() {
        return None;
    }
    let mut m = Map::new();
    if let Some(v) = &b.line1 {
        m.insert("line1".into(), Value::String(v.clone()));
    }
    if let Some(v) = &b.line2 {
        m.insert("line2".into(), Value::String(v.clone()));
    }
    if let Some(v) = &b.city {
        m.insert("city".into(), Value::String(v.clone()));
    }
    if let Some(v) = &b.state_name {
        m.insert("stateName".into(), Value::String(v.clone()));
    }
    if let Some(v) = b.state_id {
        m.insert("stateId".into(), json!(v));
    }
    if let Some(v) = &b.postal_code {
        m.insert("postalCode".into(), Value::String(v.clone()));
    }
    if let Some(v) = b.country_id {
        m.insert("countryId".into(), json!(v));
    }
    Some(Value::Object(m))
}

/// Build the **customer** `billingAddress` object, or `None` if no field was
/// supplied. Keys: `addressLine1`/`addressLine2`/`city`/`zip`/`stateName`/`stateId`/`countryId`.
pub(crate) fn billing_customer_json(b: &BillingArgs) -> Option<Value> {
    if !b.is_present() {
        return None;
    }
    let mut m = Map::new();
    if let Some(v) = &b.line1 {
        m.insert("addressLine1".into(), Value::String(v.clone()));
    }
    if let Some(v) = &b.line2 {
        m.insert("addressLine2".into(), Value::String(v.clone()));
    }
    if let Some(v) = &b.city {
        m.insert("city".into(), Value::String(v.clone()));
    }
    if let Some(v) = &b.postal_code {
        m.insert("zip".into(), Value::String(v.clone()));
    }
    if let Some(v) = &b.state_name {
        m.insert("stateName".into(), Value::String(v.clone()));
    }
    if let Some(v) = b.state_id {
        m.insert("stateId".into(), json!(v));
    }
    if let Some(v) = b.country_id {
        m.insert("countryId".into(), json!(v));
    }
    Some(Value::Object(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full() -> BillingArgs {
        BillingArgs {
            line1: Some("123 Test St".into()),
            line2: Some("Suite 4".into()),
            city: Some("Denver".into()),
            state_name: Some("CO".into()),
            state_id: Some(6),
            postal_code: Some("80202".into()),
            country_id: Some(1),
        }
    }

    #[test]
    fn is_present_false_when_all_none() {
        assert!(!BillingArgs::default().is_present());
    }

    #[test]
    fn is_present_true_with_any_field() {
        let b = BillingArgs {
            city: Some("Denver".into()),
            ..Default::default()
        };
        assert!(b.is_present());
    }

    #[test]
    fn transaction_json_none_when_empty() {
        assert!(billing_transaction_json(&BillingArgs::default()).is_none());
    }

    #[test]
    fn transaction_json_uses_transaction_keys() {
        let v = billing_transaction_json(&full()).expect("some");
        assert_eq!(v["line1"], "123 Test St");
        assert_eq!(v["line2"], "Suite 4");
        assert_eq!(v["city"], "Denver");
        assert_eq!(v["stateName"], "CO");
        assert_eq!(v["stateId"], 6);
        assert_eq!(v["postalCode"], "80202");
        assert_eq!(v["countryId"], 1);
        // must NOT use the customer spelling
        assert!(v.get("addressLine1").is_none());
        assert!(v.get("zip").is_none());
    }

    #[test]
    fn customer_json_none_when_empty() {
        assert!(billing_customer_json(&BillingArgs::default()).is_none());
    }

    #[test]
    fn customer_json_uses_customer_keys() {
        let v = billing_customer_json(&full()).expect("some");
        assert_eq!(v["addressLine1"], "123 Test St");
        assert_eq!(v["addressLine2"], "Suite 4");
        assert_eq!(v["city"], "Denver");
        assert_eq!(v["zip"], "80202");
        assert_eq!(v["stateName"], "CO");
        assert_eq!(v["stateId"], 6);
        assert_eq!(v["countryId"], 1);
        // must NOT use the transaction spelling
        assert!(v.get("line1").is_none());
        assert!(v.get("postalCode").is_none());
    }

    #[test]
    fn partial_only_emits_provided_fields() {
        let b = BillingArgs {
            city: Some("Denver".into()),
            country_id: Some(1),
            ..Default::default()
        };
        let v = billing_transaction_json(&b).expect("some");
        assert_eq!(v["city"], "Denver");
        assert_eq!(v["countryId"], 1);
        assert!(v.get("line1").is_none());
    }
}
