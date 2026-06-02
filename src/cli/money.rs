//! Parse and validate user-supplied money amounts. Never `f64` math.

use anyhow::{Result, bail};
use rust_decimal::Decimal;
use std::str::FromStr;

/// Parse `--amount`/`--tip-amount`. Rejects negatives, non-numbers, and scale > 2.
///
/// The Flute API expects a plain decimal string (e.g. `"12.50"`).
/// Scientific notation (`1e2`, `1E2`) and a leading `+` are therefore
/// rejected explicitly before we hand off to the `Decimal` parser, so the
/// policy is visible in code rather than an accident of the library's
/// parsing rules.
pub fn parse_amount(raw: &str) -> Result<Decimal> {
    let s = raw.trim();
    if s.contains('e') || s.contains('E') {
        bail!("amount must be a plain decimal, not scientific notation: {raw}");
    }
    if s.starts_with('+') {
        bail!("amount must be a plain decimal without a leading '+': {raw}");
    }
    let d = Decimal::from_str(s).map_err(|_| anyhow::anyhow!("invalid amount: {raw}"))?;
    if d.is_sign_negative() {
        bail!("amount must not be negative: {raw}");
    }
    if d.scale() > 2 {
        bail!("amount must have at most 2 decimal places: {raw}");
    }
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn parses_two_decimal_amount() {
        assert_eq!(
            parse_amount("100.00").unwrap(),
            Decimal::from_str("100.00").unwrap()
        );
    }

    #[test]
    fn rejects_negative() {
        assert!(parse_amount("-5.00").is_err());
    }

    #[test]
    fn rejects_more_than_two_dp() {
        assert!(parse_amount("1.005").is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_amount("abc").is_err());
    }

    #[test]
    fn accepts_integer_amount() {
        assert_eq!(
            parse_amount("100").unwrap(),
            Decimal::from_str("100").unwrap()
        );
    }

    #[test]
    fn accepts_zero_amount() {
        assert_eq!(
            parse_amount("0.00").unwrap(),
            Decimal::from_str("0.00").unwrap()
        );
        assert!(parse_amount("0").is_ok());
    }

    #[test]
    fn rejects_scientific_notation() {
        assert!(parse_amount("1e2").is_err());
        assert!(parse_amount("1E2").is_err());
    }

    #[test]
    fn rejects_leading_plus() {
        assert!(parse_amount("+5.00").is_err());
    }
}
