//! Integration smoke tests for Phase 0.
//!
//! Part A: wiremock-based tests that exercise the full OAuth2 token-fetch →
//!         authenticated-request stack over a real (loopback) HTTP server.
//!
//! Part B: `assert_cmd` binary tests for deterministic, network-free paths.
//!         These do not touch the OS keychain or any live endpoint.

// ─── Part A: full auth → request stack over wiremock ────────────────────────

/// Prove that a real OAuth token fetch followed by an authenticated ping works
/// end-to-end: the token fetched from /oauth2/token flows through as the
/// Authorization header on /pay-int-api/ping.
#[tokio::test]
async fn ping_auth_stack_fetches_token_and_sends_bearer() {
    use flute_cli::api::ApiClient;
    use flute_cli::auth::token::{OAuth2Fetcher, TokenStore};
    use reqwest::Client;
    use std::sync::Arc;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path},
    };

    let server = MockServer::start().await;

    // Mock the OAuth2 token endpoint.
    Mock::given(method("POST"))
        .and(path("/oauth2/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "tok-xyz",
            "expires_in": 3600,
            "token_type": "Bearer"
        })))
        .mount(&server)
        .await;

    // Mock the ping endpoint — assert the exact bearer token flows through.
    Mock::given(method("GET"))
        .and(path("/pay-int-api/ping"))
        .and(header("authorization", "Bearer tok-xyz"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"status": "ok"})))
        .mount(&server)
        .await;

    let http = Client::new();
    let fetcher = OAuth2Fetcher::new(
        format!("{}/oauth2/token", server.uri()),
        "client-id",
        "client-secret",
        http.clone(),
    );

    let api = ApiClient {
        base_url: server.uri(),
        http,
        tokens: TokenStore::new(Arc::new(fetcher)),
    };

    let body = api.ping().await.expect("ping should succeed");
    assert_eq!(body["status"], "ok");

    // NOTE: the 401-then-200 token-refetch sequence is already covered by the
    // unit test `unauthorized_triggers_token_refresh_and_retries` in
    // src/api/client/mod.rs which exercises the same reactive-refresh code path
    // with a CountingFetcher.  Duplicating it here would add noise without
    // additional coverage, so we omit it from this integration file.
}

// ─── Part B: binary tests via assert_cmd ────────────────────────────────────

/// `flute version` exits 0 and prints the crate version.
#[test]
fn version_exits_zero_and_contains_version_string() {
    let output = assert_cmd::Command::cargo_bin("flute")
        .unwrap()
        .arg("version")
        .output()
        .unwrap();

    assert!(output.status.success(), "expected exit 0");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("0.1.1"),
        "version output should contain '0.1.1', got: {stdout:?}"
    );
}

/// `flute --output json version` exits 0 and its stdout is a valid JSON
/// Envelope (object / data / meta keys present).
#[test]
fn version_json_output_is_envelope() {
    let output = assert_cmd::Command::cargo_bin("flute")
        .unwrap()
        .args(["--output", "json", "version"])
        .output()
        .unwrap();

    assert!(output.status.success(), "expected exit 0");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let val: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");

    assert_eq!(val["object"], "version", "envelope must have object key");
    assert!(val["data"].is_object(), "envelope must have data object");
    assert!(val["meta"].is_object(), "envelope must have meta object");
}

/// `flute --help` exits 0.
#[test]
fn help_exits_zero() {
    assert_cmd::Command::cargo_bin("flute")
        .unwrap()
        .arg("--help")
        .assert()
        .success();
}

// NOTE: a `flute ping` (no-credentials) binary test is intentionally omitted.
// The outcome depends on the local OS keychain and network:
//   - With no creds stored it tries (and fails) to load from the keychain,
//     which may block, prompt, or return different errors across CI platforms.
//   - The exit-code mapping for the missing-credentials path is already
//     unit-tested via `exit_code_for` in src/cli/output.rs.
// That combination makes any binary-level ping assertion inherently flaky.
