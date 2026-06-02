//! CLI handlers for the Transactions command group (`flute transactions …`).
//!
//! Populated in Tasks 1.2+; the module is wired here so the module tree
//! compiles from the outset.

/// Parse a card expiry string in `MM/YY` or `MM/YYYY` format.
///
/// Returns `(month, 4-digit year)`.
///
/// # Rules
/// - Month must be 01–12; rejects 0 and > 12.
/// - 2-digit year is treated as 2000 + YY.
/// - 4-digit year is used as-is.
/// - Year tokens that are not exactly 2 or 4 digits are rejected.
/// - Non-numeric tokens and wrong separators are rejected.
pub fn parse_exp(s: &str) -> anyhow::Result<(u32, u32)> {
    let parts: Vec<&str> = s.splitn(2, '/').collect();
    if parts.len() != 2 {
        anyhow::bail!("expiry must be MM/YY or MM/YYYY (got '{s}')");
    }
    let month: u32 = parts[0]
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid month in expiry '{s}'"))?;
    if month == 0 || month > 12 {
        anyhow::bail!("month must be 01–12, got {month} (in '{s}')");
    }
    let year_token = parts[1];
    let year_len = year_token.len();
    if year_len != 2 && year_len != 4 {
        anyhow::bail!(
            "year must be exactly 2 digits (YY) or 4 digits (YYYY), got {year_len} digits (in '{s}')"
        );
    }
    let raw_year: u32 = year_token
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid year in expiry '{s}'"))?;
    let year = if year_len == 2 {
        2000 + raw_year
    } else {
        raw_year
    };
    Ok((month, year))
}

#[cfg(test)]
mod tests {
    use super::*;

    // TDD: written before the implementation above.

    #[test]
    fn parses_mm_yy() {
        let (month, year) = parse_exp("12/26").unwrap();
        assert_eq!(month, 12);
        assert_eq!(year, 2026);
    }

    #[test]
    fn parses_mm_yyyy() {
        let (month, year) = parse_exp("03/2027").unwrap();
        assert_eq!(month, 3);
        assert_eq!(year, 2027);
    }

    #[test]
    fn rejects_bad_month() {
        assert!(parse_exp("13/26").is_err(), "month 13 must be rejected");
        assert!(parse_exp("00/26").is_err(), "month 0 must be rejected");
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_exp("abc").is_err(), "no slash must be rejected");
        assert!(parse_exp("1226").is_err(), "no slash must be rejected");
        assert!(parse_exp("ab/cd").is_err(), "non-numeric must be rejected");
    }

    #[test]
    fn rejects_malformed_year() {
        // 3-digit year must be rejected
        assert!(
            parse_exp("12/100").is_err(),
            "3-digit year must be rejected"
        );
        // 1-digit year must be rejected
        assert!(parse_exp("12/1").is_err(), "1-digit year must be rejected");
        // Sanity: valid 2- and 4-digit years still work
        assert!(parse_exp("12/26").is_ok(), "MM/YY must still be accepted");
        assert!(
            parse_exp("12/2026").is_ok(),
            "MM/YYYY must still be accepted"
        );
    }
}
