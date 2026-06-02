//! `ApiClient` endpoint methods for the Transactions API group
//! (`/pay-api/v1/transactions/*`).
//!
//! Each method is an `impl ApiClient` extension that calls
//! `self.send(…)` from the core in `mod.rs`.

use crate::api::client::ApiClient;
use crate::api::error::ApiError;
use reqwest::Method;

impl ApiClient {
    /// POST `/pay-api/v1/transactions/sale` — charge a card immediately.
    pub async fn sale(&self, body: serde_json::Value) -> Result<serde_json::Value, ApiError> {
        self.send(Method::POST, "/pay-api/v1/transactions/sale", Some(body))
            .await
    }

    /// POST `/pay-api/v1/transactions/auth` — authorise (hold) without capture.
    ///
    /// Named `auth_txn` to avoid a name collision with the auth-module concept.
    pub async fn auth_txn(&self, body: serde_json::Value) -> Result<serde_json::Value, ApiError> {
        self.send(Method::POST, "/pay-api/v1/transactions/auth", Some(body))
            .await
    }
}

#[cfg(test)]
mod tests {
    use crate::api::client::test_client;
    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, header, method, path},
    };

    /// Wiremock test: `sale` POSTs to the correct path, sends the body
    /// (including `accountNumber`), and deserialises the response correctly.
    #[tokio::test]
    async fn sale_posts_to_correct_path_with_body() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/transactions/sale"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(json!({
                "accountNumber": "4111111111111111"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "transactionId": "t_123",
                "status": "Approved"
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let body = json!({
            "amount": 100.00,
            "accountNumber": "4111111111111111",
            "cardDataSource": 1,
            "customerInitiatedTransaction": false
        });

        let result = api.sale(body).await.unwrap();
        assert_eq!(result["transactionId"], "t_123");
        assert_eq!(result["status"], "Approved");
    }

    /// Wiremock test: `auth_txn` POSTs to the correct path.
    #[tokio::test]
    async fn auth_txn_posts_to_correct_path() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/transactions/auth"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "transactionId": "t_auth_456",
                "status": "Authorized"
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let body = json!({
            "amount": 50.00,
            "cardDataSource": 1,
            "customerInitiatedTransaction": false
        });

        let result = api.auth_txn(body).await.unwrap();
        assert_eq!(result["transactionId"], "t_auth_456");
        assert_eq!(result["status"], "Authorized");
    }
}
