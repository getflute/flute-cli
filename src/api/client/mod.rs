//! Shared HTTP request core for the Flute API. Per-resource endpoint methods
//! live in sibling modules (transactions.rs, ach.rs, …).

mod ach;
mod customers;
mod devices;
mod pos;
mod settlements;
mod subscriptions;
mod terminals;
mod tokens;
mod transactions;

use crate::api::error::{ApiError, from_aspnet};
use crate::auth::token::TokenStore;
use reqwest::header::ACCEPT;
use reqwest::{Client, Method, RequestBuilder, StatusCode};
use tracing::{debug, info};

const JSON: &str = "application/json";

/// Keys whose values are sensitive authentication data that must NEVER appear
/// in logs in any form. PCI-DSS forbids storing/logging the card verification
/// value (CVV/CVC) at all, so these are replaced wholesale rather than masked.
fn is_full_secret_key(lower_key: &str) -> bool {
    matches!(
        lower_key,
        "securitycode" | "cvv" | "cvc" | "cvv2" | "cardverificationvalue"
    )
}

/// Keys carrying a full card/bank account identifier. These are logged masked
/// to the last four characters (PCI-DSS permits showing at most the last four
/// digits of a PAN), which keeps logs useful for support without exposing the
/// full number.
fn is_account_like_key(lower_key: &str) -> bool {
    matches!(
        lower_key,
        "cardnumber" | "accountnumber" | "pan" | "routingnumber"
    )
}

/// Mask a string value to its last four characters (UTF-8 safe). Values of four
/// or fewer characters are fully redacted so short inputs aren't echoed whole.
/// Non-string values are returned unchanged.
fn mask_last4(value: &serde_json::Value) -> serde_json::Value {
    match value.as_str() {
        Some(s) => {
            let chars: Vec<char> = s.chars().collect();
            if chars.len() > 4 {
                let masked: String = std::iter::repeat_n('*', chars.len() - 4)
                    .chain(chars[chars.len() - 4..].iter().copied())
                    .collect();
                serde_json::Value::String(masked)
            } else {
                serde_json::Value::String("***".into())
            }
        }
        None => value.clone(),
    }
}

/// Recursively redact sensitive fields in a JSON value so it is safe to log.
/// Secrets (CVV) become `"***"`; account/card numbers are masked to last-4;
/// everything else is preserved. Recurses through objects and arrays so nested
/// payloads are covered too.
fn redact_value(value: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            let redacted = map
                .iter()
                .map(|(k, v)| {
                    let lk = k.to_ascii_lowercase();
                    let rv = if is_full_secret_key(&lk) {
                        Value::String("***".into())
                    } else if is_account_like_key(&lk) {
                        mask_last4(v)
                    } else {
                        redact_value(v)
                    };
                    (k.clone(), rv)
                })
                .collect();
            Value::Object(redacted)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_value).collect()),
        other => other.clone(),
    }
}

/// Render a request body for debug logging with sensitive fields masked.
fn redact_for_log(body: &serde_json::Value) -> String {
    redact_value(body).to_string()
}

/// Render a response body (raw server text) for debug logging. When the text
/// parses as JSON, sensitive fields are masked; otherwise the text is returned
/// unchanged (non-JSON error pages don't carry structured card data).
fn redact_text_for_log(text: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(v) => redact_value(&v).to_string(),
        Err(_) => text.to_string(),
    }
}

#[derive(Clone)]
pub struct ApiClient {
    pub base_url: String,
    pub http: Client,
    pub tokens: TokenStore,
}

impl ApiClient {
    /// Build the request with bearer auth + Accept header + optional JSON body.
    /// Extracted so the same request can be issued twice (once with the cached
    /// token, once with a fresh token after a 401).
    fn build_request(
        &self,
        method: &Method,
        url: &str,
        body: Option<&serde_json::Value>,
        token: &str,
    ) -> RequestBuilder {
        let mut req = self
            .http
            .request(method.clone(), url)
            .bearer_auth(token)
            // Accept: application/json on every request so the ASP.NET content
            // negotiation pipeline always returns JSON (and never falls into a
            // different format-handler that has its own bugs). Content-Type is
            // set for us by .json() when a body is present.
            .header(ACCEPT, JSON);
        match (body, method) {
            (Some(b), _) => {
                req = req.json(b);
            }
            // Bodyless POST/PUT/PATCH: explicitly send an empty body so reqwest
            // emits Content-Length: 0. The Flute API rejects bodyless POSTs
            // without it ("POST requests require a Content-length"), which hit
            // the ping and retry endpoints.
            (None, m) if matches!(*m, Method::POST | Method::PUT | Method::PATCH) => {
                req = req.body("").header(reqwest::header::CONTENT_LENGTH, "0");
            }
            (None, _) => {}
        }
        req
    }

    /// Issue the request once, returning (status, body_text). Used by both
    /// send() and send_no_body() so the 401-retry logic stays in one place.
    async fn issue(
        &self,
        method: &Method,
        url: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<(StatusCode, String), ApiError> {
        let token = self
            .tokens
            .bearer()
            .await
            .map_err(|e| ApiError::Auth(e.to_string()))?;
        let resp = self.build_request(method, url, body, &token).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        Ok((status, text))
    }

    pub(crate) async fn send<R: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<R, ApiError> {
        let url = format!("{}{}", self.base_url, path);
        // Body is logged at debug level with sensitive fields masked (see
        // redact_for_log); the bearer token lives in a header and is never logged.
        debug!(method = %method, url = %url, body = ?body.as_ref().map(redact_for_log), "HTTP request");

        let (mut status, mut text) = self.issue(&method, &url, body.as_ref()).await?;
        debug!(method = %method, url = %url, status = status.as_u16(), body = %redact_text_for_log(&text), "HTTP response");

        // Reactive token refresh: a 401 may mean our cached token is stale
        // (clock skew, server restart, revocation). Drop the cache, fetch a
        // fresh token, and retry the same request once.
        if status == StatusCode::UNAUTHORIZED {
            info!("HTTP 401 — invalidating cached token and retrying once");
            self.tokens.invalidate().await;
            let (s2, t2) = self.issue(&method, &url, body.as_ref()).await?;
            debug!(method = %method, url = %url, status = s2.as_u16(), body = %redact_text_for_log(&t2), "HTTP response (after refresh)");
            status = s2;
            text = t2;
        }

        if status.is_success() {
            serde_json::from_str::<R>(&text).map_err(|e| ApiError::Decode(e.to_string()))
        } else {
            Err(from_aspnet(status.as_u16(), &text))
        }
    }

    /// Like `send_no_body` but accepts a JSON body — used when the server returns
    /// an empty 200 body (e.g. `PUT /pay-api/v1/customers/{id}`).  The response
    /// body is intentionally discarded; callers that need the updated resource
    /// should issue a subsequent GET.
    pub(crate) async fn send_body_discard(
        &self,
        method: Method,
        path: &str,
        body: serde_json::Value,
    ) -> Result<(), ApiError> {
        let url = format!("{}{}", self.base_url, path);
        debug!(method = %method, url = %url, body = %redact_for_log(&body), "HTTP request");

        let (mut status, mut text) = self.issue(&method, &url, Some(&body)).await?;

        if status == StatusCode::UNAUTHORIZED {
            info!("HTTP 401 — invalidating cached token and retrying once");
            self.tokens.invalidate().await;
            let (s2, t2) = self.issue(&method, &url, Some(&body)).await?;
            status = s2;
            text = t2;
        }

        if status.is_success() {
            debug!(method = %method, url = %url, status = status.as_u16(), "HTTP response (body discarded)");
            Ok(())
        } else {
            debug!(
                method = %method, url = %url, status = status.as_u16(),
                body = %redact_text_for_log(&text),
                "HTTP response"
            );
            Err(from_aspnet(status.as_u16(), &text))
        }
    }

    pub(crate) async fn send_no_body(&self, method: Method, path: &str) -> Result<(), ApiError> {
        let url = format!("{}{}", self.base_url, path);
        debug!(method = %method, url = %url, "HTTP request");

        let (mut status, mut text) = self.issue(&method, &url, None).await?;

        if status == StatusCode::UNAUTHORIZED {
            info!("HTTP 401 — invalidating cached token and retrying once");
            self.tokens.invalidate().await;
            let (s2, t2) = self.issue(&method, &url, None).await?;
            status = s2;
            text = t2;
        }

        if status.is_success() {
            debug!(method = %method, url = %url, status = status.as_u16(), "HTTP response (no body)");
            Ok(())
        } else {
            debug!(
                method = %method, url = %url, status = status.as_u16(),
                body = %redact_text_for_log(&text),
                "HTTP response"
            );
            Err(from_aspnet(status.as_u16(), &text))
        }
    }

    /// Health check. Returns the raw JSON body so the smoke test stays robust
    /// against the exact PingControllerPingResponse shape.
    pub async fn ping(&self) -> Result<serde_json::Value, ApiError> {
        self.send(Method::GET, "/pay-int-api/ping", None).await
    }
}

#[cfg(test)]
pub(crate) fn test_client(base_url: String) -> ApiClient {
    use crate::auth::token::{Fetcher, TokenStore};
    use std::sync::Arc;
    use std::time::Duration;

    struct Static;

    #[async_trait::async_trait]
    impl Fetcher for Static {
        async fn fetch(&self) -> anyhow::Result<(String, Duration)> {
            Ok(("test-token".into(), Duration::from_secs(3600)))
        }
    }

    ApiClient {
        base_url,
        http: Client::new(),
        tokens: TokenStore::new(Arc::new(Static)),
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn ping_hits_pay_int_api_ping_and_returns_body() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{header, method, path},
        };
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/pay-int-api/ping"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"status": "ok"})),
            )
            .mount(&server)
            .await;

        let api = super::test_client(server.uri());
        let body = api.ping().await.unwrap();
        assert_eq!(body["status"], "ok");
    }

    #[tokio::test]
    async fn unauthorized_triggers_token_refresh_and_retries() {
        use crate::auth::token::{Fetcher, TokenStore};
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Duration;
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{method, path},
        };

        // A fetcher that counts how many times fetch() is called and returns
        // tokens token-0, token-1, … so we can verify it was called twice.
        struct CountingFetcher {
            calls: Arc<AtomicUsize>,
        }

        #[async_trait::async_trait]
        impl Fetcher for CountingFetcher {
            async fn fetch(&self) -> anyhow::Result<(String, Duration)> {
                let n = self.calls.fetch_add(1, Ordering::SeqCst);
                Ok((format!("token-{n}"), Duration::from_secs(3600)))
            }
        }

        let counter = Arc::new(AtomicUsize::new(0));
        let fetcher = CountingFetcher {
            calls: counter.clone(),
        };
        let tokens = TokenStore::new(Arc::new(fetcher));

        let server = MockServer::start().await;

        // First request → 401 (fires once only).
        Mock::given(method("GET"))
            .and(path("/pay-int-api/ping"))
            .respond_with(ResponseTemplate::new(401))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        // Second request (after token refresh) → 200 with body.
        Mock::given(method("GET"))
            .and(path("/pay-int-api/ping"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"status": "ok"})),
            )
            .mount(&server)
            .await;

        let api = super::ApiClient {
            base_url: server.uri(),
            http: reqwest::Client::new(),
            tokens,
        };

        let body = api.ping().await.unwrap();
        assert_eq!(body["status"], "ok");
        // fetch() must have been called once for the initial request and once
        // after invalidate() — proving the 401-retry path works end-to-end.
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    /// Regression: send_body_discard returns Ok(()) when the server responds
    /// with HTTP 200 and an empty body (the live PUT /customers/{id} behavior
    /// that previously caused "EOF while parsing a value" via send::<Value>).
    #[tokio::test]
    async fn send_body_discard_tolerates_empty_200_body() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{body_partial_json, header, method, path},
        };
        let server = MockServer::start().await;

        // Respond with 200 and NO body — mirrors the live PUT customers endpoint.
        Mock::given(method("PUT"))
            .and(path("/pay-api/v1/customers/cust-001"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(
                serde_json::json!({"email": "new@example.com"}),
            ))
            .respond_with(ResponseTemplate::new(200)) // empty body
            .mount(&server)
            .await;

        let api = super::test_client(server.uri());
        let result = api
            .send_body_discard(
                reqwest::Method::PUT,
                "/pay-api/v1/customers/cust-001",
                serde_json::json!({"email": "new@example.com"}),
            )
            .await;

        assert!(
            result.is_ok(),
            "send_body_discard must succeed on empty 200 body, got: {result:?}"
        );
    }

    // ── Redaction of sensitive fields in debug logs ─────────────────────────
    //
    // `--debug` logs HTTP request/response bodies. Card PANs, CVVs, and bank
    // account numbers MUST be masked before they reach stderr (PCI-DSS: never
    // log full PAN, never store/log CVV). These tests pin that contract.

    use super::{redact_for_log, redact_text_for_log};
    use serde_json::json;

    #[test]
    fn redact_for_log_masks_pan_keeping_last4() {
        let body = json!({ "accountNumber": "4111111111111111" });
        let out = redact_for_log(&body);
        assert!(
            !out.contains("4111111111111111"),
            "full PAN must not appear in log output: {out}"
        );
        assert!(
            out.contains("************1111"),
            "PAN last-4 expected: {out}"
        );
    }

    #[test]
    fn redact_for_log_masks_card_number_field() {
        let body = json!({ "cardNumber": "5555444433332222" });
        let out = redact_for_log(&body);
        assert!(!out.contains("5555444433332222"), "full PAN leaked: {out}");
        assert!(out.contains("************2222"), "expected last-4: {out}");
    }

    #[test]
    fn redact_for_log_fully_redacts_cvv() {
        let body = json!({ "securityCode": "123" });
        let out = redact_for_log(&body);
        assert!(
            !out.contains("\"123\""),
            "CVV value must be fully removed, not masked-with-last-4: {out}"
        );
        assert!(out.contains("\"***\""), "CVV should become ***: {out}");
    }

    #[test]
    fn redact_for_log_masks_routing_number() {
        let body = json!({ "routingNumber": "021000021" });
        let out = redact_for_log(&body);
        assert!(!out.contains("021000021"), "full routing leaked: {out}");
    }

    #[test]
    fn redact_for_log_preserves_non_sensitive_fields() {
        let body = json!({
            "amount": "10.00",
            "currencyId": 840,
            "customerId": "cust-001",
            "cardDataSource": "Keyed"
        });
        let out = redact_for_log(&body);
        assert!(out.contains("10.00"), "amount must survive: {out}");
        assert!(out.contains("840"), "currencyId must survive: {out}");
        assert!(out.contains("cust-001"), "customerId must survive: {out}");
        assert!(out.contains("Keyed"), "cardDataSource must survive: {out}");
    }

    #[test]
    fn redact_for_log_recurses_into_nested_objects_and_arrays() {
        let body = json!({
            "paymentMethods": [
                { "card": { "cardNumber": "4111111111111111", "securityCode": "999" } }
            ]
        });
        let out = redact_for_log(&body);
        assert!(
            !out.contains("4111111111111111"),
            "nested PAN leaked: {out}"
        );
        assert!(!out.contains("\"999\""), "nested CVV leaked: {out}");
    }

    #[test]
    fn redact_for_log_short_account_does_not_reveal_value() {
        // A value of <= 4 chars must not be echoed verbatim under last-4 masking.
        let body = json!({ "accountNumber": "12" });
        let out = redact_for_log(&body);
        assert!(!out.contains("\"12\""), "short account leaked: {out}");
        assert!(out.contains("\"***\""), "short value should be ***: {out}");
    }

    #[test]
    fn redact_for_log_is_case_insensitive_on_keys() {
        let body = json!({ "AccountNumber": "4111111111111111", "CVV": "123" });
        let out = redact_for_log(&body);
        assert!(!out.contains("4111111111111111"), "PAN leaked: {out}");
        assert!(!out.contains("\"123\""), "CVV leaked: {out}");
    }

    #[test]
    fn redact_text_for_log_redacts_json_response() {
        let text = r#"{"accountNumber":"4111111111111111","securityCode":"123"}"#;
        let out = redact_text_for_log(text);
        assert!(!out.contains("4111111111111111"), "PAN leaked: {out}");
        assert!(!out.contains("\"123\""), "CVV leaked: {out}");
    }

    #[test]
    fn redact_text_for_log_passes_through_non_json() {
        let text = "Internal Server Error (not json)";
        let out = redact_text_for_log(text);
        assert_eq!(out, text, "non-JSON text must be logged unchanged");
    }

    /// End-to-end guard: drive a real request through `send()` with a capturing
    /// tracing subscriber and assert the raw PAN never reaches the log sink.
    /// This protects against a future log site bypassing `redact_for_log`.
    #[tokio::test(flavor = "current_thread")]
    async fn debug_logging_masks_pan_through_the_real_send_path() {
        use std::io::Write;
        use std::sync::{Arc, Mutex};
        use tracing_subscriber::fmt::MakeWriter;
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{method, path},
        };

        #[derive(Clone)]
        struct Buf(Arc<Mutex<Vec<u8>>>);
        struct BufGuard(Arc<Mutex<Vec<u8>>>);
        impl Write for BufGuard {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        impl<'a> MakeWriter<'a> for Buf {
            type Writer = BufGuard;
            fn make_writer(&'a self) -> Self::Writer {
                BufGuard(self.0.clone())
            }
        }

        let buf = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_ansi(false)
            .with_writer(Buf(buf.clone()))
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/redact-probe"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({ "panMask": "************1111" })),
            )
            .mount(&server)
            .await;

        let api = super::test_client(server.uri());
        let _: Result<serde_json::Value, _> = api
            .send(
                reqwest::Method::POST,
                "/redact-probe",
                Some(json!({ "accountNumber": "4111111111111111", "securityCode": "123" })),
            )
            .await;

        let logged = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        // Sanity: the request was actually traced, so the asserts below aren't vacuous.
        assert!(
            logged.contains("HTTP request"),
            "expected a request trace to be captured, got: {logged}"
        );
        // The full PAN must never reach the log sink.
        assert!(
            !logged.contains("4111111111111111"),
            "full PAN leaked into debug logs: {logged}"
        );
        // Masked PAN present → proves redaction ran (not that logging was skipped).
        assert!(
            logged.contains("************1111"),
            "expected masked PAN in logs: {logged}"
        );
        // CVV redacted to *** (the only field here that maps to ***).
        assert!(
            logged.contains("***"),
            "expected CVV to be redacted to ***: {logged}"
        );
    }

    #[test]
    fn redact_for_log_on_realistic_sale_body_hides_pan_and_cvv() {
        // Mirrors the field names build_card_txn_body emits (transactions.rs).
        let body = json!({
            "amount": "10.00",
            "cardDataSource": "Keyed",
            "accountNumber": "4111111111111111",
            "securityCode": "123",
            "expirationMonth": 12,
            "expirationYear": 2027
        });
        let out = redact_for_log(&body);
        assert!(!out.contains("4111111111111111"), "PAN leaked: {out}");
        assert!(!out.contains("\"123\""), "CVV leaked: {out}");
        // Non-sensitive structure should remain for debugging value.
        assert!(out.contains("10.00") && out.contains("Keyed"));
    }
}
