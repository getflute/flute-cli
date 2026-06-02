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
}

pub fn from_aspnet(status: u16, body: &str) -> ApiError {
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
            let message = match exception {
                Some(et) => format!("{core} [{et}]"),
                None => core,
            };
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
}
