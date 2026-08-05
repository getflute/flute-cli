//! End-to-end guard (ARISE-4505): a real request through the public `ApiClient`
//! must emit **redacted** debug logs — the raw PAN/CVV never reach the log sink.
//!
//! This lives in its own integration binary on purpose. Capturing `tracing`
//! output requires installing a process-global subscriber and touching the
//! global callsite-interest cache, which the in-process parallel test runner
//! inherently races (it flaked in CI with an empty capture). A dedicated
//! single-test binary runs in an isolated process with a fresh cache, so it is
//! deterministic.

use std::io::Write;
use std::sync::{Arc, Mutex};

use flute_cli::api::ApiClient;
use flute_cli::auth::token::{OAuth2Fetcher, TokenStore};
use tracing_subscriber::fmt::MakeWriter;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A `MakeWriter` that appends everything written into a shared byte buffer.
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

#[tokio::test(flavor = "current_thread")]
async fn debug_logs_mask_pan_and_cvv_through_the_real_send_path() {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .with_writer(Buf(buf.clone()))
        .finish();
    // Global default: this is the only test in this binary, so nothing else can
    // race the callsite-interest cache.
    tracing::subscriber::set_global_default(subscriber).expect("set global subscriber");

    let server = MockServer::start().await;
    // OAuth token endpoint (TokenStore fetches a bearer before the request).
    Mock::given(method("POST"))
        .and(path("/oauth2/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "test-token",
            "expires_in": 3600,
            "token_type": "Bearer"
        })))
        .mount(&server)
        .await;
    // Customer create — carries the sensitive request body we assert on.
    Mock::given(method("POST"))
        .and(path("/pay-api/v1/customers"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": "c1" })))
        .mount(&server)
        .await;

    let fetcher = OAuth2Fetcher::new(
        format!("{}/oauth2/token", server.uri()),
        "client-id",
        "client-secret",
        reqwest::Client::new(),
    );
    let api = ApiClient {
        base_url: server.uri(),
        http: reqwest::Client::new(),
        tokens: TokenStore::new(Arc::new(fetcher)),
    };

    // `create_customer` (public) POSTs the body through the shared `send()` core,
    // which debug-logs the request body via `redact_for_log`.
    let _ = api
        .create_customer(serde_json::json!({
            "accountNumber": "4111111111111111",
            "securityCode": "123"
        }))
        .await;

    let logged = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
    // Sanity: the request was actually traced (asserts below aren't vacuous).
    assert!(
        logged.contains("HTTP request"),
        "expected a request trace to be captured, got: {logged}"
    );
    // The full PAN must never reach the log sink.
    assert!(
        !logged.contains("4111111111111111"),
        "full PAN leaked into debug logs: {logged}"
    );
    // Masked PAN present → redaction ran (not that logging was skipped).
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
