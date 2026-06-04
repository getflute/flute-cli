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

/// Parse `--tip-rate` and similar rate fields. Like [`parse_amount`] but allows
/// up to 4 decimal places instead of 2, since rates (e.g. `0.1850`) are not
/// money amounts and commonly require sub-cent precision.
///
/// Rejects: negative values, scientific notation (`1e2`), and a leading `+`.
pub fn parse_rate(raw: &str) -> Result<Decimal> {
    let s = raw.trim();
    if s.contains('e') || s.contains('E') {
        bail!("rate must be a plain decimal, not scientific notation: {raw}");
    }
    if s.starts_with('+') {
        bail!("rate must be a plain decimal without a leading '+': {raw}");
    }
    let d = Decimal::from_str(s).map_err(|_| anyhow::anyhow!("invalid rate: {raw}"))?;
    if d.is_sign_negative() {
        bail!("rate must not be negative: {raw}");
    }
    if d.scale() > 4 {
        bail!("rate must have at most 4 decimal places: {raw}");
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

    // ── parse_rate ────────────────────────────────────────────────────────────

    #[test]
    fn parse_rate_accepts_two_dp() {
        assert_eq!(
            parse_rate("18.5").unwrap(),
            Decimal::from_str("18.5").unwrap()
        );
    }

    #[test]
    fn parse_rate_accepts_four_dp() {
        assert_eq!(
            parse_rate("0.185").unwrap(),
            Decimal::from_str("0.185").unwrap()
        );
        assert_eq!(
            parse_rate("0.1850").unwrap(),
            Decimal::from_str("0.1850").unwrap()
        );
    }

    #[test]
    fn parse_rate_rejects_negative() {
        assert!(parse_rate("-0.10").is_err());
    }

    #[test]
    fn parse_rate_rejects_garbage() {
        assert!(parse_rate("not-a-rate").is_err());
    }

    #[test]
    fn parse_rate_rejects_scientific_notation() {
        assert!(parse_rate("1e2").is_err());
        assert!(parse_rate("1E2").is_err());
    }

    #[test]
    fn parse_rate_rejects_leading_plus() {
        assert!(parse_rate("+0.10").is_err());
    }

    #[test]
    fn parse_rate_rejects_more_than_four_dp() {
        assert!(parse_rate("0.12345").is_err());
    }
}
