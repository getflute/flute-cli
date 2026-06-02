//! Parse and validate user-supplied money amounts. Never `f64` math.

use anyhow::{Result, bail};
use rust_decimal::Decimal;
use std::str::FromStr;

/// Parse `--amount`/`--tip-amount`. Rejects negatives, non-numbers, and scale > 2.
pub fn parse_amount(raw: &str) -> Result<Decimal> {
    let d = Decimal::from_str(raw.trim()).map_err(|_| anyhow::anyhow!("invalid amount: {raw}"))?;
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
}
