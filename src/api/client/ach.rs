//! `ApiClient` endpoint methods for the ACH API group
//! (`/pay-api/v1/transactions/ach/*`).
//!
//! Each method calls `self.send(…)` from the core in `mod.rs`.

use crate::api::client::ApiClient;
use crate::api::error::ApiError;
use reqwest::Method;

impl ApiClient {
    /// POST `/pay-api/v1/transactions/ach/payment` — ACH debit.
    pub async fn ach_debit(&self, body: serde_json::Value) -> Result<serde_json::Value, ApiError> {
        self.send(
            Method::POST,
            "/pay-api/v1/transactions/ach/payment",
            Some(body),
        )
        .await
    }

    /// POST `/pay-api/v1/transactions/ach/payment/credit` — ACH credit.
    pub async fn ach_credit(&self, body: serde_json::Value) -> Result<serde_json::Value, ApiError> {
        self.send(
            Method::POST,
            "/pay-api/v1/transactions/ach/payment/credit",
            Some(body),
        )
        .await
    }

    /// POST `/pay-api/v1/transactions/ach/{id}/void` — void an ACH transaction (bodyless).
    pub async fn ach_void(&self, id: &str) -> Result<serde_json::Value, ApiError> {
        let path = format!("/pay-api/v1/transactions/ach/{id}/void");
        self.send(Method::POST, &path, None).await
    }

    /// POST `/pay-api/v1/transactions/ach/{id}/refund` — refund an ACH transaction (bodyless).
    pub async fn ach_refund(&self, id: &str) -> Result<serde_json::Value, ApiError> {
        let path = format!("/pay-api/v1/transactions/ach/{id}/refund");
        self.send(Method::POST, &path, None).await
    }
}

#[cfg(test)]
mod tests {
    use crate::api::client::test_client;
    use crate::cli::ach::{AccountTypeArg, AchArgs, build_ach_body};
    use rust_decimal::Decimal;
    use serde_json::json;
    use std::str::FromStr;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, body_string, header, method, path},
    };

    /// Wiremock test: `ach_debit` POSTs to the correct path.
    ///
    /// The request body is built via `build_ach_body` so a regression in the
    /// builder is caught here at the transport layer (builder → transport wiring).
    #[tokio::test]
    async fn ach_debit_posts_to_correct_path_with_body() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/transactions/ach/payment"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(json!({
                "secCode": 1,
                "paymentProcessorId": "pp-ach-001",
                "accountType": 1
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "transactionId": "ach-txn-001",
                "status": "Approved"
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let body = build_ach_body(&AchArgs {
            amount: Decimal::from_str("100.00").unwrap(),
            payment_processor_id: "pp-ach-001".into(),
            requester_ip: "127.0.0.1".into(),
            sec_code: 1,
            routing: Some("021000021".into()),
            account: Some("123456789".into()),
            account_type: Some(AccountTypeArg::Checking),
            account_holder_type: None,
            tax_id: None,
            customer_id: None,
            payment_method_id: None,
            faster: false,
        })
        .unwrap();

        let result = api.ach_debit(body).await.unwrap();
        assert_eq!(result["transactionId"], "ach-txn-001");
        assert_eq!(result["status"], "Approved");
    }

    /// Wiremock test: `ach_credit` POSTs to the correct path with body fields.
    #[tokio::test]
    async fn ach_credit_posts_to_correct_path_with_body() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/transactions/ach/payment/credit"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(json!({
                "secCode": 1,
                "paymentProcessorId": "pp-ach-002"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "transactionId": "ach-credit-001",
                "status": "Approved"
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let body = json!({
            "amount": 250.00,
            "paymentProcessorId": "pp-ach-002",
            "requesterIpAddress": "127.0.0.1",
            "secCode": 1,
            "isFasterProcessing": false
        });

        let result = api.ach_credit(body).await.unwrap();
        assert_eq!(result["transactionId"], "ach-credit-001");
        assert_eq!(result["status"], "Approved");
    }

    /// Wiremock test: `ach_void` POSTs to `/pay-api/v1/transactions/ach/{id}/void`
    /// as a bodyless POST. The `body_string("")` matcher asserts no body is sent,
    /// so a regression that accidentally attaches a payload would fail this test.
    #[tokio::test]
    async fn ach_void_posts_to_correct_path_with_empty_body() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/transactions/ach/ach-txn-void-001/void"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string(""))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "transactionId": "ach-txn-void-001",
                "status": "Voided"
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.ach_void("ach-txn-void-001").await.unwrap();
        assert_eq!(result["transactionId"], "ach-txn-void-001");
        assert_eq!(result["status"], "Voided");
    }

    /// Wiremock test: `ach_refund` POSTs to `/pay-api/v1/transactions/ach/{id}/refund`
    /// as a bodyless POST. The `body_string("")` matcher asserts no body is sent,
    /// so a regression that accidentally attaches a payload would fail this test.
    #[tokio::test]
    async fn ach_refund_posts_to_correct_path_with_empty_body() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/transactions/ach/ach-txn-ref-001/refund"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string(""))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "transactionId": "ach-txn-ref-001",
                "status": "Refunded"
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.ach_refund("ach-txn-ref-001").await.unwrap();
        assert_eq!(result["transactionId"], "ach-txn-ref-001");
        assert_eq!(result["status"], "Refunded");
    }
}
