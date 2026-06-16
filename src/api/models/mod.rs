//! API data models — re-exports and shared helpers.

pub mod common;
pub mod transactions;

/// Convert a [`rust_decimal::Decimal`] to a [`serde_json::Value::Number`] that
/// preserves the exact decimal representation on the wire.
///
/// With `serde_json`'s `arbitrary_precision` feature enabled, parsing a
/// decimal string like `"100.00"` produces a JSON `Number` that serialises
/// back as `100.00` — not `100` or `100.0` — satisfying the Flute API's
/// double-precision amount fields.
///
/// # Errors
/// Returns a `serde_json::Error` if serde_json rejects the decimal literal.
/// This should not happen for `rust_decimal::Decimal`, but callers propagate it
/// rather than panicking.
pub fn to_amount_number(d: rust_decimal::Decimal) -> serde_json::Result<serde_json::Value> {
    // `Decimal::to_string()` emits the exact decimal string (e.g. "100.00",
    // "0.10") without scientific notation and without float rounding.
    // With `serde_json`'s `arbitrary_precision` feature enabled, parsing that
    // string produces a JSON Number whose serialised form is identical — i.e.
    // "100.00" → the JSON number 100.00, not 100 or 100.0.
    serde_json::from_str::<serde_json::Number>(&d.to_string()).map(serde_json::Value::Number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    // TDD: these tests must be written and pass green before the helper is used
    // by any endpoint builder.

    #[test]
    fn to_amount_number_is_a_json_number_not_a_string() {
        let v = to_amount_number(Decimal::from_str("100.00").unwrap()).unwrap();
        assert!(v.is_number(), "expected a JSON number, got {v:?}");
    }

    #[test]
    fn to_amount_number_preserves_exact_decimal() {
        let v = to_amount_number(Decimal::from_str("100.00").unwrap()).unwrap();
        let s = serde_json::to_string(&v).unwrap();
        // Must be the bare number 100.00 — not "100.00" (string), not 100 or 100.0 (float artifact)
        assert_eq!(s, "100.00", "expected serialised form '100.00', got '{s}'");
    }

    #[test]
    fn to_amount_number_zero_point_ten_no_float_artifact() {
        let v = to_amount_number(Decimal::from_str("0.10").unwrap()).unwrap();
        let s = serde_json::to_string(&v).unwrap();
        // Must NOT become a float artifact like 0.10000000000000001.
        // With arbitrary_precision, Decimal("0.10").to_string() = "0.10",
        // so the round-trip is exact.
        assert_eq!(
            s, "0.10",
            "expected '0.10', got '{s}' — arbitrary_precision must preserve decimal precision"
        );
        assert!(v.is_number());
    }
}
