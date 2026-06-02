//! Output contract: JSON envelope, agent error envelope, exit codes, table/quiet.

use crate::api::error::ApiError;
use serde::Serialize;

#[derive(Copy, Clone, Debug, clap::ValueEnum, PartialEq, Eq)]
pub enum OutputFormat {
    Table,
    Json,
    Quiet,
}

#[derive(Debug, Serialize)]
pub struct Meta {
    pub environment: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Envelope<T: Serialize> {
    pub object: &'static str,
    pub data: T,
    pub meta: Meta,
}

impl<T: Serialize> Envelope<T> {
    pub fn new(
        object: &'static str,
        data: T,
        environment: &str,
        correlation_id: Option<String>,
    ) -> Self {
        Self {
            object,
            data,
            meta: Meta {
                environment: environment.to_string(),
                correlation_id,
            },
        }
    }
}

/// Agent error envelope — printed to stdout under --output json on failure.
///
/// `kind` is one of:
///   - `"api"`       — HTTP error from the Flute API (status + correlation_id present)
///   - `"transport"` — connection / TLS / DNS failure (status absent)
///   - `"auth"`      — OAuth or keychain failure (status absent)
///   - `"decode"`    — request encode or response decode failure (status absent)
///   - `"client"`    — anything else (config, CLI parsing, unknown)
#[derive(Debug, Serialize)]
pub struct ErrorJson {
    pub kind: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

impl ErrorJson {
    /// Build a structured envelope by inspecting the error chain for an
    /// `ApiError` (the typed variant we can extract richer fields from).
    /// Anything else falls back to `kind="client"` with the formatted
    /// message — the agent still gets *a* JSON object, never bare text.
    pub fn from_anyhow(err: &anyhow::Error) -> Self {
        if let Some(api) = err.downcast_ref::<ApiError>() {
            return match api {
                ApiError::Api {
                    status,
                    correlation_id,
                    message,
                } => Self {
                    kind: "api",
                    message: message.clone(),
                    status: Some(*status),
                    correlation_id: correlation_id.clone(),
                },
                ApiError::Transport(e) => Self {
                    kind: "transport",
                    message: e.to_string(),
                    status: None,
                    correlation_id: None,
                },
                ApiError::Auth(m) => Self {
                    kind: "auth",
                    message: m.clone(),
                    status: None,
                    correlation_id: None,
                },
                ApiError::Decode(m) => Self {
                    kind: "decode",
                    message: m.clone(),
                    status: None,
                    correlation_id: None,
                },
            };
        }
        Self {
            kind: "client",
            message: err.to_string(),
            status: None,
            correlation_id: None,
        }
    }
}

/// Map an HTTP status to the spec's semantic exit code.
pub fn exit_code_for_api(status: u16) -> i32 {
    match status {
        401 | 403 => 2,
        400 | 422 => 3,
        404 => 4,
        _ => 1,
    }
}

/// Derive the process exit code from a failed anyhow error.
pub fn exit_code_for(err: &anyhow::Error) -> i32 {
    match err.downcast_ref::<ApiError>() {
        Some(ApiError::Api { status, .. }) => exit_code_for_api(*status),
        Some(ApiError::Auth(_)) => 2,
        Some(ApiError::Decode(_)) | Some(ApiError::Transport(_)) => 1,
        None => 1,
    }
}

/// Trim/pad a string to a column width with an ellipsis (table mode helper).
#[allow(dead_code)]
pub(crate) fn fit(s: &str, width: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= width {
        format!("{s:<width$}")
    } else if width >= 1 {
        let kept: String = chars.into_iter().take(width.saturating_sub(1)).collect();
        format!("{kept}…")
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::error::ApiError;

    // --- Plan tests ---

    #[test]
    fn envelope_wraps_data_with_meta() {
        let env = Envelope::new(
            "transaction",
            serde_json::json!({"id":"t1"}),
            "sandbox",
            Some("c-1".into()),
        );
        let v = serde_json::to_value(&env).unwrap();
        assert_eq!(v["object"], "transaction");
        assert_eq!(v["data"]["id"], "t1");
        assert_eq!(v["meta"]["environment"], "sandbox");
        assert_eq!(v["meta"]["correlation_id"], "c-1");
    }

    #[test]
    fn error_json_api_variant_surfaces_status_and_correlation_id() {
        let e: anyhow::Error = ApiError::Api {
            status: 422,
            correlation_id: Some("abc".into()),
            message: "bad".into(),
        }
        .into();
        let env = ErrorJson::from_anyhow(&e);
        assert_eq!(env.kind, "api");
        assert_eq!(env.status, Some(422));
        assert_eq!(env.correlation_id.as_deref(), Some("abc"));
    }

    #[test]
    fn exit_code_maps_status_classes() {
        assert_eq!(exit_code_for_api(401), 2);
        assert_eq!(exit_code_for_api(403), 2);
        assert_eq!(exit_code_for_api(404), 4);
        assert_eq!(exit_code_for_api(400), 3);
        assert_eq!(exit_code_for_api(422), 3);
        assert_eq!(exit_code_for_api(500), 1);
    }

    // --- Reference tests (ported from flute-webhooks) ---

    #[test]
    fn fit_pads_short_strings_to_width() {
        assert_eq!(fit("hi", 5), "hi   ");
    }

    #[test]
    fn fit_truncates_long_strings_with_ellipsis() {
        let out = fit("hello-world", 6);
        assert_eq!(out.chars().count(), 6);
        assert!(out.ends_with('…'));
        assert!(out.starts_with("hello"));
    }

    #[test]
    fn error_json_auth_variant_uses_auth_kind_no_status() {
        let e: anyhow::Error = ApiError::Auth("no token".into()).into();
        let env = ErrorJson::from_anyhow(&e);
        assert_eq!(env.kind, "auth");
        assert!(env.status.is_none());
        assert!(env.correlation_id.is_none());
        assert_eq!(env.message, "no token");
        // status / correlation_id must be omitted from JSON (not null) so
        // agents that branch on key-presence stay consistent across kinds.
        let json = serde_json::to_string(&env).unwrap();
        assert!(!json.contains("status"));
        assert!(!json.contains("correlation_id"));
    }

    #[test]
    fn error_json_falls_back_to_client_for_unknown_errors() {
        let e = anyhow::anyhow!("unknown profile: garbage");
        let env = ErrorJson::from_anyhow(&e);
        assert_eq!(env.kind, "client");
        assert!(env.message.contains("garbage"));
    }

    #[test]
    fn exit_code_for_routes_apierror_variants() {
        // Auth → 2
        let auth_err: anyhow::Error = ApiError::Auth("no token".into()).into();
        assert_eq!(exit_code_for(&auth_err), 2);

        // Decode → 1
        let decode_err: anyhow::Error = ApiError::Decode("bad json".into()).into();
        assert_eq!(exit_code_for(&decode_err), 1);

        // Api with 404 → 4
        let api_404: anyhow::Error = ApiError::Api {
            status: 404,
            correlation_id: None,
            message: "not found".into(),
        }
        .into();
        assert_eq!(exit_code_for(&api_404), 4);

        // Plain anyhow (non-ApiError) → 1
        let plain = anyhow::anyhow!("x");
        assert_eq!(exit_code_for(&plain), 1);
    }

    #[test]
    fn error_json_finds_api_error_through_context_wrapper() {
        // `?` callers often add context via `anyhow::Context`; the downcast
        // must still locate the typed ApiError in the chain, otherwise the
        // envelope would lose status + correlation_id.
        let inner: anyhow::Error = ApiError::Api {
            status: 500,
            correlation_id: Some("xyz".into()),
            message: "boom".into(),
        }
        .into();
        let wrapped = inner.context("while creating endpoint");
        let env = ErrorJson::from_anyhow(&wrapped);
        assert_eq!(env.kind, "api");
        assert_eq!(env.status, Some(500));
        assert_eq!(env.correlation_id.as_deref(), Some("xyz"));
    }
}
