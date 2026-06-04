//! `ApiClient` endpoint methods for the Customers / Vault API group
//! (`/pay-api/v1/customers/*`).
//!
//! Each method calls `self.send(…)` or `self.send_no_body(…)` from the core
//! in `mod.rs`.  Bodyless DELETEs use `send_no_body` which returns `()`.

use crate::api::client::ApiClient;
use crate::api::error::ApiError;
use reqwest::Method;
use url::form_urlencoded;

impl ApiClient {
    /// POST `/pay-api/v1/customers` — create a new customer.
    pub async fn create_customer(
        &self,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
        self.send(Method::POST, "/pay-api/v1/customers", Some(body))
            .await
    }

    /// GET `/pay-api/v1/customers/{id}` — fetch a single customer by ID.
    pub async fn get_customer(&self, id: &str) -> Result<serde_json::Value, ApiError> {
        let path = format!("/pay-api/v1/customers/{id}");
        self.send(Method::GET, &path, None).await
    }

    /// GET `/pay-api/v1/customers` — list customers with optional pagination and search.
    ///
    /// Only query parameters that are `Some` are appended to the URL.
    ///
    /// - `page`      → `page` query param
    /// - `page_size` → `pageSize` query param
    /// - `search`    → `search` query param (server-side text search)
    pub async fn list_customers(
        &self,
        page: Option<u32>,
        page_size: Option<u32>,
        search: Option<String>,
    ) -> Result<serde_json::Value, ApiError> {
        let mut serializer = form_urlencoded::Serializer::new(String::new());
        if let Some(p) = page {
            serializer.append_pair("page", &p.to_string());
        }
        if let Some(ps) = page_size {
            serializer.append_pair("pageSize", &ps.to_string());
        }
        if let Some(ref s) = search {
            serializer.append_pair("search", s);
        }
        let qs = serializer.finish();

        let path = if qs.is_empty() {
            "/pay-api/v1/customers".to_string()
        } else {
            format!("/pay-api/v1/customers?{qs}")
        };

        self.send(Method::GET, &path, None).await
    }

    /// PUT `/pay-api/v1/customers/{id}` — update a customer.
    ///
    /// NOTE: The API may treat PUT as a full replacement; callers should pass
    /// only the fields the user explicitly supplied (and document the risk of
    /// unintended field reset until GET-merge-PUT is implemented).
    pub async fn update_customer(
        &self,
        id: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
        let path = format!("/pay-api/v1/customers/{id}");
        self.send(Method::PUT, &path, Some(body)).await
    }

    /// DELETE `/pay-api/v1/customers/{id}` — delete a customer (bodyless).
    pub async fn delete_customer(&self, id: &str) -> Result<(), ApiError> {
        let path = format!("/pay-api/v1/customers/{id}");
        self.send_no_body(Method::DELETE, &path).await
    }

    /// POST `/pay-api/v1/customers/{id}/payment-methods/cards` — vault a card.
    pub async fn add_card(
        &self,
        id: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
        let path = format!("/pay-api/v1/customers/{id}/payment-methods/cards");
        self.send(Method::POST, &path, Some(body)).await
    }

    /// POST `/pay-api/v1/customers/{id}/payment-methods/ach` — vault an ACH account.
    pub async fn add_ach(
        &self,
        id: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
        let path = format!("/pay-api/v1/customers/{id}/payment-methods/ach");
        self.send(Method::POST, &path, Some(body)).await
    }

    /// GET `/pay-api/v1/customers/{id}/payment-methods` — list payment methods for a customer.
    pub async fn list_payment_methods(&self, id: &str) -> Result<serde_json::Value, ApiError> {
        let path = format!("/pay-api/v1/customers/{id}/payment-methods");
        self.send(Method::GET, &path, None).await
    }

    /// DELETE `/pay-api/v1/customers/{id}/payment-methods/{mid}` — remove a payment method.
    pub async fn remove_payment_method(&self, id: &str, mid: &str) -> Result<(), ApiError> {
        let path = format!("/pay-api/v1/customers/{id}/payment-methods/{mid}");
        self.send_no_body(Method::DELETE, &path).await
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::api::client::test_client;
    use crate::cli::customers::build_customer_body;
    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, header, method, path, query_param},
    };

    fn sample_customer() -> serde_json::Value {
        json!({
            "id": "cust-001",
            "firstName": "Alice",
            "lastName": "Smith",
            "email": "alice@example.com",
            "mobilePhoneNumber": "5550001234",
            "companyName": null,
            "createdOn": "2024-01-15T10:00:00Z"
        })
    }

    fn sample_pm() -> serde_json::Value {
        json!({
            "id": "pm-001",
            "typeName": "Visa",
            "panMask": "411111****1111",
            "isDefault": true
        })
    }

    // ── create_customer ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn create_customer_posts_to_correct_path_with_body() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/customers"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(json!({"email": "alice@example.com"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_customer()))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        // Build via the canonical builder to prove builder→transport wiring.
        let body = build_customer_body(
            Some("Alice"),
            Some("Smith"),
            None,
            Some("alice@example.com"),
            None,
        );
        let result = api.create_customer(body).await.unwrap();
        assert_eq!(result["id"], "cust-001");
        assert_eq!(result["firstName"], "Alice");
    }

    // ── get_customer ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn get_customer_hits_correct_path() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pay-api/v1/customers/cust-001"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_customer()))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.get_customer("cust-001").await.unwrap();
        assert_eq!(result["id"], "cust-001");
    }

    // ── list_customers ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_customers_no_params_hits_base_path() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pay-api/v1/customers"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [sample_customer()],
                "total": 1
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.list_customers(None, None, None).await.unwrap();
        assert_eq!(result["total"], 1);
        assert_eq!(result["items"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn list_customers_passes_page_size_query_param() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pay-api/v1/customers"))
            .and(query_param("pageSize", "10"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [],
                "total": 0
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.list_customers(None, Some(10), None).await.unwrap();
        assert_eq!(result["total"], 0);
    }

    #[tokio::test]
    async fn list_customers_passes_search_query_param() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pay-api/v1/customers"))
            .and(query_param("search", "alice"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [sample_customer()],
                "total": 1
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api
            .list_customers(None, None, Some("alice".into()))
            .await
            .unwrap();
        assert_eq!(result["total"], 1);
    }

    /// Confirm that a space in the search value is percent-encoded (+ or %20)
    /// and that wiremock decodes it back to the raw string for matching.
    #[tokio::test]
    async fn list_customers_url_encodes_space_in_search() {
        let server = MockServer::start().await;

        // wiremock's query_param matcher decodes via url::Url::query_pairs(),
        // so we match on the decoded string "alice smith" — the transport will
        // have sent "alice+smith" or "alice%20smith" on the wire.
        Mock::given(method("GET"))
            .and(path("/pay-api/v1/customers"))
            .and(query_param("search", "alice smith"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [sample_customer()],
                "total": 1
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api
            .list_customers(None, None, Some("alice smith".into()))
            .await
            .unwrap();
        assert_eq!(result["total"], 1);

        // Also assert the raw query string on the wire contains an encoding of the space.
        let received = server.received_requests().await.unwrap();
        assert_eq!(received.len(), 1);
        let raw_query = received[0].url.query().unwrap_or("");
        // form_urlencoded encodes space as '+'; percent_encode uses %20
        assert!(
            raw_query.contains("alice+smith") || raw_query.contains("alice%20smith"),
            "expected encoded space in query, got: {raw_query}"
        );
    }

    // ── update_customer ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn update_customer_puts_to_correct_path_with_body() {
        let server = MockServer::start().await;

        Mock::given(method("PUT"))
            .and(path("/pay-api/v1/customers/cust-001"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(json!({"email": "updated@example.com"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_customer()))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let body = json!({"email": "updated@example.com"});
        let result = api.update_customer("cust-001", body).await.unwrap();
        assert_eq!(result["id"], "cust-001");
    }

    // ── delete_customer ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn delete_customer_deletes_to_correct_path_returns_unit() {
        let server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/pay-api/v1/customers/cust-001"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.delete_customer("cust-001").await;
        assert!(result.is_ok());
    }

    // ── add_card ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn add_card_posts_to_correct_path_with_body() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/customers/cust-001/payment-methods/cards"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(json!({"pan": "4111111111111111"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_pm()))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let body =
            json!({"pan": "4111111111111111", "expirationMonth": 12, "expirationYear": 2026});
        let result = api.add_card("cust-001", body).await.unwrap();
        assert_eq!(result["id"], "pm-001");
    }

    // ── add_ach ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn add_ach_posts_to_correct_path_with_body() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/customers/cust-001/payment-methods/ach"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(json!({"accountType": 1})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "pm-ach-001",
                "typeName": "ACH"
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let body = json!({
            "accountNumber": "123456789",
            "routingNumber": "021000021",
            "accountType": 1
        });
        let result = api.add_ach("cust-001", body).await.unwrap();
        assert_eq!(result["id"], "pm-ach-001");
    }

    // ── list_payment_methods ──────────────────────────────────────────────────

    #[tokio::test]
    async fn list_payment_methods_hits_correct_path() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pay-api/v1/customers/cust-001/payment-methods"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([sample_pm()])))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.list_payment_methods("cust-001").await.unwrap();
        assert!(result.as_array().is_some());
        assert_eq!(result.as_array().unwrap().len(), 1);
        assert_eq!(result[0]["id"], "pm-001");
    }

    // ── remove_payment_method ─────────────────────────────────────────────────

    #[tokio::test]
    async fn remove_payment_method_deletes_to_correct_path_returns_unit() {
        let server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path(
                "/pay-api/v1/customers/cust-001/payment-methods/pm-001",
            ))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.remove_payment_method("cust-001", "pm-001").await;
        assert!(result.is_ok());
    }
}
