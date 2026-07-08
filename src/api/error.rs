//! API error type and ASP.NET error envelope parsing.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("transport error: {0}")]
    Transport(#[from] reqwest::Error),

    #[error("API {status} (correlation_id={correlation_id:?}): {message}")]
    Api {
        status: u16,
        correlation_id: Option<String>,
        message: String,
    },

    #[error("auth error: {0}")]
    Auth(String),

    #[error("invalid response: {0}")]
    Decode(String),
}

// The Flute API returns errors in two casings: camelCase from the public-API
// layer and PascalCase from internal exception handlers (e.g. 500s with
// "Title", "CorrelationId" capitalized). Accept both via serde alias so we
// extract everything regardless of response shape.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct AspNetError {
    #[serde(alias = "Details")]
    pub details: Option<String>,
    #[serde(alias = "Title")]
    pub title: Option<String>,
    #[serde(rename = "correlationId", alias = "CorrelationId")]
    pub correlation_id: Option<String>,
    #[serde(rename = "errorCode", alias = "ErrorCode")]
    #[allow(dead_code)]
    pub error_code: Option<String>,
    #[serde(rename = "exceptionType", alias = "ExceptionType")]
    pub exception_type: Option<String>,
    // Field-level validation errors: a map of field name → messages. The
    // empty-string key holds form-level (non-field) messages. A BTreeMap keeps
    // the flattened output deterministic. These carry the *actionable* detail
    // (e.g. "CurrencyId must not be empty") that the generic Title/Details miss.
    #[serde(rename = "errors", alias = "Errors")]
    pub errors: Option<std::collections::BTreeMap<String, Vec<String>>>,
}

impl AspNetError {
    /// Flatten the field-error map into a readable, deterministic string:
    /// `"field: msg; msg2"`. Empty-key (form-level) messages omit the prefix.
    fn flatten_errors(&self) -> Option<String> {
        let map = self.errors.as_ref()?;
        let parts: Vec<String> = map
            .iter()
            .flat_map(|(field, msgs)| {
                msgs.iter().map(move |m| {
                    if field.is_empty() {
                        m.clone()
                    } else {
                        format!("{field}: {m}")
                    }
                })
            })
            .collect();
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("; "))
        }
    }
}

pub(crate) fn from_aspnet(status: u16, body: &str) -> ApiError {
    match serde_json::from_str::<AspNetError>(body) {
        Ok(e) => {
            // Title is often generic (e.g. "Internal server error") — combine
            // with Details so the actual cause survives. Append ExceptionType
            // when present because the .NET exception class is a strong
            // diagnostic signal (e.g. "ArgumentNullException").
            let title = e.title.as_deref().filter(|s| !s.is_empty());
            let details = e.details.as_deref().filter(|s| !s.is_empty());
            let exception = e.exception_type.as_deref().filter(|s| !s.is_empty());
            let core = match (title, details) {
                (Some(t), Some(d)) => format!("{t}: {d}"),
                (Some(t), None) => t.to_string(),
                (None, Some(d)) => d.to_string(),
                (None, None) => body.to_string(),
            };
            let mut message = match exception {
                Some(et) => format!("{core} [{et}]"),
                None => core,
            };
            // Append the actionable field-level validation errors when present.
            if let Some(fields) = e.flatten_errors() {
                message = format!("{message}: {fields}");
            }
            ApiError::Api {
                status,
                correlation_id: e.correlation_id,
                message,
            }
        }
        Err(_) => ApiError::Api {
            status,
            correlation_id: None,
            message: body.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_combines_title_and_details() {
        let body = r#"{"details":"X","title":"Validation failed","statusCode":400,"correlationId":"abc-123","errorCode":"V0000"}"#;
        match from_aspnet(400, body) {
            ApiError::Api {
                status,
                correlation_id,
                message,
            } => {
                assert_eq!(status, 400);
                assert_eq!(correlation_id.as_deref(), Some("abc-123"));
                // Both fields surface — title alone would mask the specific cause.
                assert_eq!(message, "Validation failed: X");
            }
            _ => panic!("expected Api"),
        }
    }

    #[test]
    fn message_uses_details_when_title_missing() {
        let body = r#"{"details":"X","correlationId":"abc-123"}"#;
        match from_aspnet(400, body) {
            ApiError::Api { message, .. } => assert_eq!(message, "X"),
            _ => panic!(),
        }
    }

    #[test]
    fn falls_back_when_body_is_not_aspnet() {
        match from_aspnet(500, "oops") {
            ApiError::Api {
                status,
                correlation_id,
                message,
            } => {
                assert_eq!(status, 500);
                assert!(correlation_id.is_none());
                assert_eq!(message, "oops");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn surfaces_field_level_validation_errors() {
        // Real 400 body from the live sandbox: the actionable detail is in the
        // `Errors` map, not Title/Details. The parser must surface it so the
        // operator/agent doesn't need --debug to see what failed.
        let body = r#"{"Errors":{"":["Card expiration year must be provided and be in [2000..3000] range"],"CurrencyId":["'Currency Id' must not be empty."]},"Details":"One or more validation errors occurred.","StatusCode":400,"ExceptionType":"ValidationException","CorrelationId":"e828","ErrorCode":"V0000","Title":"Validation failed"}"#;
        match from_aspnet(400, body) {
            ApiError::Api {
                message,
                correlation_id,
                ..
            } => {
                assert_eq!(correlation_id.as_deref(), Some("e828"));
                // Field errors appended after the core message; BTreeMap order
                // puts the empty-key (form-level) message before "CurrencyId".
                assert!(
                    message.contains("Validation failed: One or more validation errors occurred. [ValidationException]"),
                    "core message preserved: {message}"
                );
                assert!(
                    message.contains("Card expiration year must be provided"),
                    "form-level error surfaced: {message}"
                );
                assert!(
                    message.contains("CurrencyId: 'Currency Id' must not be empty."),
                    "field-prefixed error surfaced: {message}"
                );
            }
            _ => panic!("expected Api"),
        }
    }

    #[test]
    fn pascal_case_500_surfaces_full_message_with_exception_type() {
        // Internal 500s from the API come back PascalCase. Title alone is a
        // generic "Internal server error" — the actionable info is in Details
        // and ExceptionType. The parser must surface all three.
        let body = r#"{"Details":"Value cannot be null. (Parameter 'uriString')","StatusCode":500,"Source":"IsvApiBff","ExceptionType":"ArgumentNullException","CorrelationId":"45d859f6-dc38-4d8f-8bab-ae4e20036919","ErrorCode":"I0000","Title":"Internal server error"}"#;
        match from_aspnet(500, body) {
            ApiError::Api {
                status,
                correlation_id,
                message,
            } => {
                assert_eq!(status, 500);
                assert_eq!(
                    correlation_id.as_deref(),
                    Some("45d859f6-dc38-4d8f-8bab-ae4e20036919")
                );
                assert_eq!(
                    message,
                    "Internal server error: Value cannot be null. (Parameter 'uriString') [ArgumentNullException]"
                );
            }
            _ => panic!("expected Api"),
        }
    }

    #[test]
    fn body_is_used_verbatim_when_title_and_details_missing() {
        // Valid ASP.NET JSON that carries a correlation id but no Title and no
        // Details: the parser has no human message to build, so it must fall
        // back to the raw body rather than emitting an empty message.
        let body = r#"{"correlationId":"c-1"}"#;
        match from_aspnet(500, body) {
            ApiError::Api {
                status,
                correlation_id,
                message,
            } => {
                assert_eq!(status, 500);
                assert_eq!(correlation_id.as_deref(), Some("c-1"));
                assert_eq!(message, body, "raw body preserved as the message");
            }
            _ => panic!("expected Api"),
        }
    }

    #[test]
    fn empty_field_errors_map_appends_nothing() {
        // The `Errors` map is present but every field's message list is empty,
        // so `flatten_errors` yields nothing and the core message is unchanged
        // (no dangling ": " suffix).
        let body = r#"{"Title":"X","Errors":{"CurrencyId":[]}}"#;
        match from_aspnet(400, body) {
            ApiError::Api { message, .. } => {
                assert_eq!(message, "X");
                assert!(
                    !message.contains("CurrencyId"),
                    "empty field lists must not be appended: {message}"
                );
            }
            _ => panic!("expected Api"),
        }
    }
}
