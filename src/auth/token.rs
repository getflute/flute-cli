//! OAuth2 `client_credentials` token store with in-memory caching.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct TokenStore {
    inner: Arc<Mutex<Option<CachedToken>>>,
    fetcher: Arc<dyn Fetcher + Send + Sync>,
}

#[derive(Debug, Clone)]
struct CachedToken {
    bearer: String,
    expires_at: Instant,
}

#[async_trait::async_trait]
pub trait Fetcher {
    async fn fetch(&self) -> anyhow::Result<(String, Duration)>;
}

impl TokenStore {
    pub fn new(fetcher: Arc<dyn Fetcher + Send + Sync>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
            fetcher,
        }
    }

    pub async fn bearer(&self) -> anyhow::Result<String> {
        let mut guard = self.inner.lock().await;
        if let Some(cached) = guard.as_ref() {
            // Refresh 60s before actual expiry
            if cached.expires_at.saturating_duration_since(Instant::now()) > Duration::from_secs(60)
            {
                return Ok(cached.bearer.clone());
            }
        }
        let (bearer, ttl) = self.fetcher.fetch().await?;
        let cached = CachedToken {
            bearer: bearer.clone(),
            expires_at: Instant::now() + ttl,
        };
        *guard = Some(cached);
        Ok(bearer)
    }

    /// Discard the cached token. The next [`bearer`](Self::bearer) call will
    /// fetch fresh credentials. Used for reactive refresh when the server
    /// returns 401 (clock skew, revocation, server restart) so a single stale
    /// cache entry doesn't keep failing requests.
    pub async fn invalidate(&self) {
        let mut guard = self.inner.lock().await;
        *guard = None;
    }
}

pub struct OAuth2Fetcher {
    pub(crate) oauth_url: String,
    pub(crate) client_id: String,
    pub(crate) client_secret: String,
    pub(crate) http: reqwest::Client,
}

impl OAuth2Fetcher {
    /// Public constructor so integration tests (and external callers) can
    /// build an `OAuth2Fetcher` without accessing the `pub(crate)` fields.
    pub fn new(
        oauth_url: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        http: reqwest::Client,
    ) -> Self {
        Self {
            oauth_url: oauth_url.into(),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            http,
        }
    }
}

#[derive(serde::Deserialize)]
struct TokenResp {
    access_token: String,
    expires_in: u64,
}

#[async_trait::async_trait]
impl Fetcher for OAuth2Fetcher {
    async fn fetch(&self) -> anyhow::Result<(String, Duration)> {
        let resp: TokenResp = self
            .http
            .post(&self.oauth_url)
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", &self.client_id),
                ("client_secret", &self.client_secret),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok((resp.access_token, Duration::from_secs(resp.expires_in)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    struct CountingFetcher {
        calls: AtomicUsize,
        ttl: Duration,
    }

    #[async_trait::async_trait]
    impl Fetcher for CountingFetcher {
        async fn fetch(&self) -> anyhow::Result<(String, Duration)> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            Ok((format!("token-{n}"), self.ttl))
        }
    }

    #[tokio::test]
    async fn caches_token_within_validity() {
        let f = Arc::new(CountingFetcher {
            calls: AtomicUsize::new(0),
            ttl: Duration::from_secs(3600),
        });
        let store = TokenStore::new(f.clone());
        assert_eq!(store.bearer().await.unwrap(), "token-0");
        assert_eq!(store.bearer().await.unwrap(), "token-0");
        assert_eq!(f.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn invalidate_forces_a_refetch_on_next_bearer_call() {
        let f = Arc::new(CountingFetcher {
            calls: AtomicUsize::new(0),
            ttl: Duration::from_secs(3600),
        });
        let store = TokenStore::new(f.clone());
        assert_eq!(store.bearer().await.unwrap(), "token-0");
        store.invalidate().await;
        assert_eq!(store.bearer().await.unwrap(), "token-1");
        assert_eq!(f.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn refreshes_when_within_60s_of_expiry() {
        let f = Arc::new(CountingFetcher {
            calls: AtomicUsize::new(0),
            ttl: Duration::from_secs(30),
        });
        let store = TokenStore::new(f.clone());
        assert_eq!(store.bearer().await.unwrap(), "token-0");
        // 30s ttl is below the 60s safety margin, so the next call refreshes
        assert_eq!(store.bearer().await.unwrap(), "token-1");
    }

    #[tokio::test]
    async fn oauth2_fetcher_parses_token_response() {
        use wiremock::matchers::{body_string_contains, method};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_string_contains("grant_type=client_credentials"))
            .and(body_string_contains("client_id=test-client-id"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "abc.def.ghi",
                "expires_in": 3600,
                "token_type": "Bearer"
            })))
            .mount(&server)
            .await;

        let fetcher = OAuth2Fetcher {
            oauth_url: format!("{}/oauth2/token", server.uri()),
            client_id: "test-client-id".into(),
            client_secret: "test-secret".into(),
            http: reqwest::Client::new(),
        };
        let (bearer, ttl) = fetcher.fetch().await.unwrap();
        assert_eq!(bearer, "abc.def.ghi");
        assert_eq!(ttl, Duration::from_secs(3600));
    }
}
