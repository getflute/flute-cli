//! `ApiClient` endpoint methods for the ISV Tokens API group
//! (`/pay-api/v1/merchants/tokens`).
//!
//! Each method calls `self.send(…)` or `self.send_no_body(…)` from the core
//! in `mod.rs`.  Bodyless DELETE uses `send_no_body` which returns `()`.

use crate::api::client::ApiClient;
use crate::api::error::ApiError;
use reqwest::Method;
use url::form_urlencoded;

impl ApiClient {
    /// POST `/pay-api/v1/merchants/tokens` — create an ISV API token.
    ///
    /// `body` must contain `merchantId` and `tokenName`.
    /// The response contains `clientId` and `clientSecret` (shown **only once**).
    pub async fn create_token(
        &self,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
        self.send(Method::POST, "/pay-api/v1/merchants/tokens", Some(body))
            .await
    }

    /// GET `/pay-api/v1/merchants/tokens` — list ISV API tokens.
    ///
    /// `merchant_id` is optional; when present it is sent as `merchantId` query param.
    /// Response shape: `{tokens: [...]}`.
    pub async fn list_tokens(
        &self,
        merchant_id: Option<&str>,
    ) -> Result<serde_json::Value, ApiError> {
        let path = if let Some(mid) = merchant_id {
            let mut serializer = form_urlencoded::Serializer::new(String::new());
            serializer.append_pair("merchantId", mid);
            let qs = serializer.finish();
            format!("/pay-api/v1/merchants/tokens?{qs}")
        } else {
            "/pay-api/v1/merchants/tokens".to_string()
        };

        self.send(Method::GET, &path, None).await
    }

    /// DELETE `/pay-api/v1/merchants/tokens/{clientId}?merchantId=<id>` — revoke an ISV token (bodyless).
    ///
    /// `merchant_id` is **required** by the API; omitting it returns `400 "MerchantId: Wrong merchant."`.
    pub async fn revoke_token(&self, client_id: &str, merchant_id: &str) -> Result<(), ApiError> {
        let mut serializer = form_urlencoded::Serializer::new(String::new());
        serializer.append_pair("merchantId", merchant_id);
        let qs = serializer.finish();
        let path = format!("/pay-api/v1/merchants/tokens/{client_id}?{qs}");
        self.send_no_body(Method::DELETE, &path).await
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::api::client::test_client;
    use crate::cli::tokens::build_token_body;
    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, header, method, path, query_param},
    };

    fn sample_token_create_response() -> serde_json::Value {
        json!({
            "clientId": "client-abc-123",
            "clientSecret": "super-secret-value-shown-once"
        })
    }

    fn sample_token_list_item() -> serde_json::Value {
        json!({
            "clientId": "client-abc-123",
            "tokenName": "My App Token",
            "merchantId": "merchant-001",
            "creationDate": "2024-03-15T10:00:00Z"
        })
    }

    // ── create_token ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn create_token_posts_to_correct_path_with_body() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/merchants/tokens"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(json!({
                "merchantId": "merchant-001",
                "tokenName": "My App Token"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_token_create_response()))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let body = build_token_body("merchant-001", "My App Token");
        let result = api.create_token(body).await.unwrap();
        assert_eq!(result["clientId"], "client-abc-123");
        assert_eq!(result["clientSecret"], "super-secret-value-shown-once");
    }

    // ── list_tokens ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_tokens_no_merchant_id_hits_base_path() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pay-api/v1/merchants/tokens"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "tokens": [sample_token_list_item()]
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.list_tokens(None).await.unwrap();
        assert!(result.get("tokens").is_some());
        assert_eq!(result["tokens"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn list_tokens_passes_merchant_id_query_param() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pay-api/v1/merchants/tokens"))
            .and(query_param("merchantId", "merchant-001"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "tokens": [sample_token_list_item()]
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.list_tokens(Some("merchant-001")).await.unwrap();
        assert_eq!(result["tokens"].as_array().unwrap().len(), 1);
    }

    // ── revoke_token ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn revoke_token_deletes_to_correct_path_with_merchant_id_query_param() {
        let server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/pay-api/v1/merchants/tokens/client-abc-123"))
            .and(query_param("merchantId", "merchant-001"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.revoke_token("client-abc-123", "merchant-001").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn revoke_token_404_treated_as_ok_via_treat_404_as_ok() {
        let server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/pay-api/v1/merchants/tokens/client-gone"))
            .and(query_param("merchantId", "merchant-001"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({
                "title": "Not Found",
                "status": 404
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        // The raw call returns an Err(ApiError::Api{status:404,...})
        let result = api.revoke_token("client-gone", "merchant-001").await;
        // The error is a 404 — treat_404_as_ok will convert it to Ok
        let converted = crate::treat_404_as_ok(result);
        assert!(
            converted.is_ok(),
            "404 on revoke must be treated as idempotent success via treat_404_as_ok"
        );
    }
}
