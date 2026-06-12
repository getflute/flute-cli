//! `ApiClient` endpoint methods for the Subscriptions API group
//! (`/sub-api/v1/subscriptions/*`).
//!
//! Each method calls `self.send(…)` or `self.send_no_body(…)` from the core
//! in `mod.rs`.  The terminate endpoint is a bodyless PUT that returns a body.

use crate::api::client::ApiClient;
use crate::api::error::ApiError;
use reqwest::Method;
use url::form_urlencoded;

impl ApiClient {
    /// POST `/sub-api/v1/subscriptions` — create a new subscription.
    pub async fn create_subscription(
        &self,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
        self.send(Method::POST, "/sub-api/v1/subscriptions", Some(body))
            .await
    }

    /// GET `/sub-api/v1/subscriptions/{id}` — fetch a single subscription by ID.
    pub async fn get_subscription(&self, id: &str) -> Result<serde_json::Value, ApiError> {
        let path = format!("/sub-api/v1/subscriptions/{id}");
        self.send(Method::GET, &path, None).await
    }

    /// GET `/sub-api/v1/subscriptions` — list subscriptions with optional pagination and filters.
    ///
    /// Only query parameters that are `Some` are appended to the URL.
    ///
    /// - `page`        → `page` query param
    /// - `page_size`   → `pageSize` query param
    /// - `search`      → `search` query param (server-side text search)
    /// - `customer_id` → `customerIds` query param (filter by customer UUID)
    pub async fn list_subscriptions(
        &self,
        page: Option<u32>,
        page_size: Option<u32>,
        search: Option<&str>,
        customer_id: Option<&str>,
    ) -> Result<serde_json::Value, ApiError> {
        let mut serializer = form_urlencoded::Serializer::new(String::new());
        if let Some(p) = page {
            serializer.append_pair("page", &p.to_string());
        }
        if let Some(ps) = page_size {
            serializer.append_pair("pageSize", &ps.to_string());
        }
        if let Some(s) = search {
            serializer.append_pair("search", s);
        }
        if let Some(cid) = customer_id {
            serializer.append_pair("customerIds", cid);
        }
        let qs = serializer.finish();

        let path = if qs.is_empty() {
            "/sub-api/v1/subscriptions".to_string()
        } else {
            format!("/sub-api/v1/subscriptions?{qs}")
        };

        self.send(Method::GET, &path, None).await
    }

    /// GET `/sub-api/v1/subscriptions/{id}/payments` — list payments for a subscription.
    pub async fn subscription_payments(&self, id: &str) -> Result<serde_json::Value, ApiError> {
        let path = format!("/sub-api/v1/subscriptions/{id}/payments");
        self.send(Method::GET, &path, None).await
    }

    /// PUT `/sub-api/v1/subscriptions/{id}/terminate` — terminate a subscription (bodyless PUT).
    ///
    /// The server returns a body with the updated subscription state.
    pub async fn terminate_subscription(&self, id: &str) -> Result<serde_json::Value, ApiError> {
        let path = format!("/sub-api/v1/subscriptions/{id}/terminate");
        // Bodyless PUT: send with no body but still expect a JSON response body.
        self.send(Method::PUT, &path, None).await
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::api::client::test_client;
    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path, query_param},
    };

    fn sample_subscription() -> serde_json::Value {
        json!({
            "id": "sub-001",
            "status": "Active",
            "customerId": "cust-001",
            "paymentAmount": 49.99,
            "paymentFrequencyUnit": 3,
            "paymentFrequency": 1,
            "numberOfPayments": 12,
            "successfulPaymentsCount": 2,
            "nextPaymentDate": "2026-07-01T00:00:00Z"
        })
    }

    fn sample_subscription_list_item() -> serde_json::Value {
        json!({
            "subscriptionId": "sub-001",
            "customerName": "Alice Smith",
            "amountPerPayment": 49.99,
            "paymentFrequencyUnit": 3,
            "paymentFrequency": 1,
            "status": "Active",
            "nextPaymentDate": "2026-07-01",
            "successfulPaymentsCount": 2,
            "numberOfPayments": 12
        })
    }

    fn sample_payment() -> serde_json::Value {
        json!({
            "id": "pay-001",
            "status": "Successful",
            "amount": 49.99,
            "paymentOrder": 1,
            "initialExecutionDateTime": "2026-05-01T10:00:00Z",
            "attempts": 1
        })
    }

    // ── create_subscription ───────────────────────────────────────────────────

    #[tokio::test]
    async fn create_subscription_posts_to_correct_path_with_body() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/sub-api/v1/subscriptions"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_subscription()))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let body = json!({
            "customerId": "cust-001",
            "paymentMethodId": "pm-001",
            "amount": 49.99,
            "numberOfPayments": 12,
            "requesterIpAddress": "127.0.0.1"
        });
        let result = api.create_subscription(body).await.unwrap();
        assert_eq!(result["id"], "sub-001");
        assert_eq!(result["status"], "Active");
    }

    // ── get_subscription ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn get_subscription_hits_correct_path() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/sub-api/v1/subscriptions/sub-001"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_subscription()))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.get_subscription("sub-001").await.unwrap();
        assert_eq!(result["id"], "sub-001");
    }

    // ── list_subscriptions ────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_subscriptions_no_params_hits_base_path() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/sub-api/v1/subscriptions"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [sample_subscription_list_item()],
                "total": 1
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api
            .list_subscriptions(None, None, None, None)
            .await
            .unwrap();
        assert_eq!(result["total"], 1);
        assert_eq!(result["items"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn list_subscriptions_passes_page_size_and_search_query_params() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/sub-api/v1/subscriptions"))
            .and(query_param("pageSize", "10"))
            .and(query_param("search", "alice"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [sample_subscription_list_item()],
                "total": 1
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api
            .list_subscriptions(None, Some(10), Some("alice"), None)
            .await
            .unwrap();
        assert_eq!(result["total"], 1);
    }

    #[tokio::test]
    async fn list_subscriptions_passes_customer_id_query_param() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/sub-api/v1/subscriptions"))
            .and(query_param("customerIds", "cust-001"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [sample_subscription_list_item()],
                "total": 1
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api
            .list_subscriptions(None, None, None, Some("cust-001"))
            .await
            .unwrap();
        assert_eq!(result["total"], 1);
    }

    // ── subscription_payments ─────────────────────────────────────────────────

    #[tokio::test]
    async fn subscription_payments_hits_correct_path() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/sub-api/v1/subscriptions/sub-001/payments"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([sample_payment()])))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.subscription_payments("sub-001").await.unwrap();
        assert!(result.as_array().is_some());
        assert_eq!(result.as_array().unwrap().len(), 1);
        assert_eq!(result[0]["id"], "pay-001");
    }

    // ── terminate_subscription ────────────────────────────────────────────────

    #[tokio::test]
    async fn terminate_subscription_puts_to_correct_path_returns_body() {
        let server = MockServer::start().await;

        Mock::given(method("PUT"))
            .and(path("/sub-api/v1/subscriptions/sub-001/terminate"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "sub-001",
                "status": "Terminated"
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.terminate_subscription("sub-001").await.unwrap();
        assert_eq!(result["id"], "sub-001");
        assert_eq!(result["status"], "Terminated");
    }
}
