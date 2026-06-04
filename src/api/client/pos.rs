//! `ApiClient` endpoint methods for the POS Transactions API group
//! (`/pos-api/v1/pos-transactions/*`).
//!
//! Each method calls `self.send(…)` from the core in `mod.rs`.

use crate::api::client::ApiClient;
use crate::api::error::ApiError;
use reqwest::Method;
use url::form_urlencoded;

impl ApiClient {
    /// POST `/pos-api/v1/pos-transactions` — create a POS transaction.
    pub async fn create_pos_transaction(
        &self,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
        self.send(Method::POST, "/pos-api/v1/pos-transactions", Some(body))
            .await
    }

    /// GET `/pos-api/v1/pos-transactions/{id}` — fetch a single POS transaction.
    pub async fn get_pos_transaction(&self, id: &str) -> Result<serde_json::Value, ApiError> {
        let path = format!("/pos-api/v1/pos-transactions/{id}");
        self.send(Method::GET, &path, None).await
    }

    /// GET `/pos-api/v1/pos-transactions` — list POS transactions with optional pagination
    /// and terminal filter.
    ///
    /// - `page`        → `page` query param
    /// - `page_size`   → `pageSize` query param
    /// - `terminal_id` → `terminalId` query param
    pub async fn list_pos_transactions(
        &self,
        page: Option<u32>,
        page_size: Option<u32>,
        terminal_id: Option<&str>,
    ) -> Result<serde_json::Value, ApiError> {
        let mut serializer = form_urlencoded::Serializer::new(String::new());
        if let Some(p) = page {
            serializer.append_pair("page", &p.to_string());
        }
        if let Some(ps) = page_size {
            serializer.append_pair("pageSize", &ps.to_string());
        }
        if let Some(tid) = terminal_id {
            serializer.append_pair("terminalId", tid);
        }
        let qs = serializer.finish();

        let path = if qs.is_empty() {
            "/pos-api/v1/pos-transactions".to_string()
        } else {
            format!("/pos-api/v1/pos-transactions?{qs}")
        };

        self.send(Method::GET, &path, None).await
    }

    /// POST `/pos-api/v1/pos-transactions/{id}/cancel` — cancel a POS transaction (bodyless).
    pub async fn cancel_pos_transaction(&self, id: &str) -> Result<serde_json::Value, ApiError> {
        let path = format!("/pos-api/v1/pos-transactions/{id}/cancel");
        self.send(Method::POST, &path, None).await
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::api::client::test_client;
    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, header, method, path, query_param},
    };

    fn sample_pos_txn() -> serde_json::Value {
        json!({
            "id": "pos-txn-001",
            "terminalId": "term-001",
            "transactionType": "Sale",
            "amount": 100.00,
            "currencyId": 1,
            "posTransactionStatus": "TerminalConnecting",
            "posTransactionStatusId": 1,
            "isCompleted": false,
            "transactionId": null
        })
    }

    fn sample_pos_txn_completed() -> serde_json::Value {
        json!({
            "id": "pos-txn-001",
            "terminalId": "term-001",
            "transactionType": "Sale",
            "amount": 100.00,
            "currencyId": 1,
            "posTransactionStatus": "TransactionProcessing",
            "posTransactionStatusId": 2,
            "isCompleted": true,
            "transactionId": "card-txn-abc"
        })
    }

    // ── create_pos_transaction ────────────────────────────────────────────────

    #[tokio::test]
    async fn create_pos_transaction_posts_body() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pos-api/v1/pos-transactions"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(json!({
                "terminalId": "term-001",
                "transactionTypeId": 2,
                "waitForAcceptanceByTerminal": false
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_pos_txn()))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let body = json!({
            "terminalId": "term-001",
            "transactionTypeId": 2,
            "amount": 100.00,
            "currencyId": 1,
            "waitForAcceptanceByTerminal": false
        });
        let result = api.create_pos_transaction(body).await.unwrap();
        assert_eq!(result["id"], "pos-txn-001");
        assert_eq!(result["isCompleted"], false);
    }

    // ── get_pos_transaction ───────────────────────────────────────────────────

    #[tokio::test]
    async fn get_pos_transaction_hits_correct_path() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pos-api/v1/pos-transactions/pos-txn-001"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_pos_txn_completed()))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.get_pos_transaction("pos-txn-001").await.unwrap();
        assert_eq!(result["id"], "pos-txn-001");
        assert_eq!(result["isCompleted"], true);
    }

    // ── list_pos_transactions ─────────────────────────────────────────────────

    #[tokio::test]
    async fn list_pos_transactions_no_params_hits_base_path() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pos-api/v1/pos-transactions"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [sample_pos_txn()],
                "total": 1
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.list_pos_transactions(None, None, None).await.unwrap();
        assert_eq!(result["total"], 1);
    }

    #[tokio::test]
    async fn list_pos_transactions_passes_page_size() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pos-api/v1/pos-transactions"))
            .and(query_param("pageSize", "10"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [],
                "total": 0
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api
            .list_pos_transactions(None, Some(10), None)
            .await
            .unwrap();
        assert_eq!(result["total"], 0);
    }

    #[tokio::test]
    async fn list_pos_transactions_passes_terminal_id() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pos-api/v1/pos-transactions"))
            .and(query_param("terminalId", "term-001"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [sample_pos_txn()],
                "total": 1
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api
            .list_pos_transactions(None, None, Some("term-001"))
            .await
            .unwrap();
        assert_eq!(result["total"], 1);
    }

    #[tokio::test]
    async fn list_pos_transactions_passes_all_params() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pos-api/v1/pos-transactions"))
            .and(query_param("page", "2"))
            .and(query_param("pageSize", "5"))
            .and(query_param("terminalId", "term-abc"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [],
                "total": 0
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api
            .list_pos_transactions(Some(2), Some(5), Some("term-abc"))
            .await
            .unwrap();
        assert_eq!(result["total"], 0);
    }

    // ── cancel_pos_transaction ────────────────────────────────────────────────

    #[tokio::test]
    async fn cancel_pos_transaction_posts_bodyless_to_correct_path() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pos-api/v1/pos-transactions/pos-txn-001/cancel"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "pos-txn-001",
                "isCompleted": true,
                "posTransactionStatus": "CancelByPos"
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.cancel_pos_transaction("pos-txn-001").await.unwrap();
        assert_eq!(result["id"], "pos-txn-001");
        assert_eq!(result["posTransactionStatus"], "CancelByPos");

        // Verify no body was sent
        let received = server.received_requests().await.unwrap();
        assert_eq!(received.len(), 1);
        assert!(received[0].body.is_empty());
    }
}
