//! Shared HTTP request core for the Flute API. Per-resource endpoint methods
//! live in sibling modules (transactions.rs, ach.rs, …).

mod transactions;

use crate::api::error::{ApiError, from_aspnet};
use crate::auth::token::TokenStore;
use reqwest::header::ACCEPT;
use reqwest::{Client, Method, RequestBuilder, StatusCode};
use tracing::{debug, info};

const JSON: &str = "application/json";

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
        // Body is logged at debug level in full; bearer token is intentionally not logged.
        let body_for_log = body.as_ref().map(|b| b.to_string());
        debug!(method = %method, url = %url, body = ?body_for_log, "HTTP request");

        let (mut status, mut text) = self.issue(&method, &url, body.as_ref()).await?;
        debug!(method = %method, url = %url, status = status.as_u16(), body = %text, "HTTP response");

        // Reactive token refresh: a 401 may mean our cached token is stale
        // (clock skew, server restart, revocation). Drop the cache, fetch a
        // fresh token, and retry the same request once.
        if status == StatusCode::UNAUTHORIZED {
            info!("HTTP 401 — invalidating cached token and retrying once");
            self.tokens.invalidate().await;
            let (s2, t2) = self.issue(&method, &url, body.as_ref()).await?;
            debug!(method = %method, url = %url, status = s2.as_u16(), body = %t2, "HTTP response (after refresh)");
            status = s2;
            text = t2;
        }

        if status.is_success() {
            serde_json::from_str::<R>(&text).map_err(|e| ApiError::Decode(e.to_string()))
        } else {
            Err(from_aspnet(status.as_u16(), &text))
        }
    }

    #[allow(dead_code)]
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
                body = %text,
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
}
