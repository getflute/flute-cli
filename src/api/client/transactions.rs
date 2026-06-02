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

    /// POST `/pay-api/v1/transactions/capture` — capture a previously authorised transaction.
    ///
    /// Pass `None` for a full capture; pass `Some(amount)` for a partial capture.
    pub async fn capture(&self, body: serde_json::Value) -> Result<serde_json::Value, ApiError> {
        self.send(Method::POST, "/pay-api/v1/transactions/capture", Some(body))
            .await
    }

    /// POST `/pay-api/v1/transactions/void` — void a transaction.
    pub async fn void(&self, body: serde_json::Value) -> Result<serde_json::Value, ApiError> {
        self.send(Method::POST, "/pay-api/v1/transactions/void", Some(body))
            .await
    }

    /// POST `/pay-api/v1/transactions/return` — refund (return) a transaction.
    ///
    /// Note: the API path is `/return`, not `/refund`.
    pub async fn refund(&self, body: serde_json::Value) -> Result<serde_json::Value, ApiError> {
        self.send(Method::POST, "/pay-api/v1/transactions/return", Some(body))
            .await
    }

    /// POST `/pay-api/v1/transactions/settle` — settle the open batch for a payment processor.
    ///
    /// **Important**: this is a batch-level operation keyed by `paymentProcessorId`, NOT a
    /// per-transaction operation. The body must contain `paymentProcessorId`.
    pub async fn settle(&self, body: serde_json::Value) -> Result<serde_json::Value, ApiError> {
        self.send(Method::POST, "/pay-api/v1/transactions/settle", Some(body))
            .await
    }

    /// POST `/pay-api/v1/transactions/tip-adjustment` — adjust the tip on a transaction.
    pub async fn tip_adjust(&self, body: serde_json::Value) -> Result<serde_json::Value, ApiError> {
        self.send(
            Method::POST,
            "/pay-api/v1/transactions/tip-adjustment",
            Some(body),
        )
        .await
    }

    /// GET `/pay-api/v1/transactions/{id}` — fetch a single transaction by ID.
    pub async fn get_transaction(&self, id: &str) -> Result<serde_json::Value, ApiError> {
        let path = format!("/pay-api/v1/transactions/{id}");
        self.send(Method::GET, &path, None).await
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

    /// Wiremock test: `auth_txn` POSTs to the correct path with the expected body fields.
    #[tokio::test]
    async fn auth_txn_posts_to_correct_path_with_body() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/transactions/auth"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(json!({
                "cardDataSource": 1,
                "customerInitiatedTransaction": false
            })))
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

    // ── Task 1.6 wiremock tests ───────────────────────────────────────────────

    /// Wiremock test: `capture` POSTs to `/pay-api/v1/transactions/capture`
    /// with `transactionId` in the body.
    #[tokio::test]
    async fn capture_posts_to_correct_path_with_transaction_id() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/transactions/capture"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(json!({
                "transactionId": "txn-cap-001"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "transactionId": "txn-cap-001",
                "status": "Captured"
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let body = json!({ "transactionId": "txn-cap-001" });

        let result = api.capture(body).await.unwrap();
        assert_eq!(result["transactionId"], "txn-cap-001");
        assert_eq!(result["status"], "Captured");
    }

    /// Wiremock test: `void` POSTs to `/pay-api/v1/transactions/void`
    /// with `transactionId` in the body.
    #[tokio::test]
    async fn void_posts_to_correct_path_with_transaction_id() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/transactions/void"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(json!({
                "transactionId": "txn-void-002"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "transactionId": "txn-void-002",
                "status": "Voided"
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let body = json!({ "transactionId": "txn-void-002" });

        let result = api.void(body).await.unwrap();
        assert_eq!(result["transactionId"], "txn-void-002");
        assert_eq!(result["status"], "Voided");
    }

    /// Wiremock test: `refund` POSTs to `/pay-api/v1/transactions/return` (not /refund).
    #[tokio::test]
    async fn refund_posts_to_return_path_with_transaction_id_and_card_data_source() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/transactions/return"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(json!({
                "transactionId": "txn-ret-003",
                "cardDataSource": 1
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "transactionId": "txn-ret-003",
                "status": "Returned"
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let body = json!({
            "transactionId": "txn-ret-003",
            "cardDataSource": 1
        });

        let result = api.refund(body).await.unwrap();
        assert_eq!(result["transactionId"], "txn-ret-003");
        assert_eq!(result["status"], "Returned");
    }

    /// Wiremock test: `settle` POSTs to `/pay-api/v1/transactions/settle`
    /// with `paymentProcessorId` (NOT transactionId) in the body.
    #[tokio::test]
    async fn settle_posts_to_correct_path_with_payment_processor_id() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/transactions/settle"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(json!({
                "paymentProcessorId": "proc-uuid-settle"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "Settled"
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let body = json!({ "paymentProcessorId": "proc-uuid-settle" });

        let result = api.settle(body).await.unwrap();
        assert_eq!(result["status"], "Settled");
    }

    /// Wiremock test: `tip_adjust` POSTs to `/pay-api/v1/transactions/tip-adjustment`
    /// with `transactionId` and `tipAmount` in the body.
    #[tokio::test]
    async fn tip_adjust_posts_to_correct_path_with_tip_amount() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/transactions/tip-adjustment"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(json!({
                "transactionId": "txn-tip-005",
                "tipAmount": 3.50
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "transactionId": "txn-tip-005",
                "status": "Approved"
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let body = json!({
            "transactionId": "txn-tip-005",
            "tipAmount": 3.50
        });

        let result = api.tip_adjust(body).await.unwrap();
        assert_eq!(result["transactionId"], "txn-tip-005");
        assert_eq!(result["status"], "Approved");
    }

    // ── Task 1.7 wiremock test ────────────────────────────────────────────────

    /// Wiremock test: `get_transaction` GETs `/pay-api/v1/transactions/{id}`
    /// and returns the transaction object.
    #[tokio::test]
    async fn get_transaction_gets_correct_path() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pay-api/v1/transactions/txn-get-abc"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "transactionId": "txn-get-abc",
                "status": "Approved",
                "amount": {
                    "totalAmount": 75.00,
                    "baseAmount": 75.00
                }
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());

        let result = api.get_transaction("txn-get-abc").await.unwrap();
        assert_eq!(result["transactionId"], "txn-get-abc");
        assert_eq!(result["status"], "Approved");
    }
}
