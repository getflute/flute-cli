//! `ApiClient` endpoint methods for the Terminals API group
//! (`/pos-api/v1/terminals/*`).
//!
//! Each method calls `self.send(…)` from the core in `mod.rs`.

use crate::api::client::ApiClient;
use crate::api::error::ApiError;
use reqwest::Method;
use url::form_urlencoded;

impl ApiClient {
    /// GET `/pos-api/v1/terminals` — list terminals with optional pagination.
    ///
    /// - `page`      → `page` query param
    /// - `page_size` → `pageSize` query param
    pub async fn list_terminals(
        &self,
        page: Option<u32>,
        page_size: Option<u32>,
    ) -> Result<serde_json::Value, ApiError> {
        let mut serializer = form_urlencoded::Serializer::new(String::new());
        if let Some(p) = page {
            serializer.append_pair("page", &p.to_string());
        }
        if let Some(ps) = page_size {
            serializer.append_pair("pageSize", &ps.to_string());
        }
        let qs = serializer.finish();

        let path = if qs.is_empty() {
            "/pos-api/v1/terminals".to_string()
        } else {
            format!("/pos-api/v1/terminals?{qs}")
        };

        self.send(Method::GET, &path, None).await
    }

    /// GET `/pos-api/v1/terminals/{id}/status` — retrieve terminal status.
    pub async fn terminal_status(&self, id: &str) -> Result<serde_json::Value, ApiError> {
        let path = format!("/pos-api/v1/terminals/{id}/status");
        self.send(Method::GET, &path, None).await
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

    fn sample_terminal() -> serde_json::Value {
        json!({
            "id": "term-001",
            "serialNumber": "SN123456",
            "terminalManufacturer": "Ingenico",
            "terminalModel": "Desk/5000",
            "terminalModeName": "Desk 5000",
            "connectionStatus": "Online",
            "deliveryStatusName": "Delivered",
            "lastSeenTimestamp": "2024-06-01T10:00:00Z"
        })
    }

    fn sample_terminal_status() -> serde_json::Value {
        json!({
            "terminalId": "term-001",
            "terminalPosStatus": "Active",
            "connectionStatus": "Online",
            "connectionType": "WiFi",
            "wifiConnectionStrength": 85,
            "batteryLevel": 72,
            "availabilityStatus": "Available",
            "ariseTerminalVersion": "2.1.0",
            "printerStatus": "Ready",
            "lastSeenTimestamp": "2024-06-01T10:00:00Z"
        })
    }

    // ── list_terminals ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_terminals_no_params_hits_base_path() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pos-api/v1/terminals"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [sample_terminal()],
                "total": 1
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.list_terminals(None, None).await.unwrap();
        assert_eq!(result["total"], 1);
        assert_eq!(result["items"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn list_terminals_passes_page_size_query_param() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pos-api/v1/terminals"))
            .and(query_param("pageSize", "10"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [],
                "total": 0
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.list_terminals(None, Some(10)).await.unwrap();
        assert_eq!(result["total"], 0);
    }

    #[tokio::test]
    async fn list_terminals_passes_page_query_param() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pos-api/v1/terminals"))
            .and(query_param("page", "2"))
            .and(query_param("pageSize", "25"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [],
                "total": 0
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.list_terminals(Some(2), Some(25)).await.unwrap();
        assert_eq!(result["total"], 0);
    }

    // ── terminal_status ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn terminal_status_hits_correct_path() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pos-api/v1/terminals/term-001/status"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_terminal_status()))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.terminal_status("term-001").await.unwrap();
        assert_eq!(result["terminalId"], "term-001");
        assert_eq!(result["terminalPosStatus"], "Active");
    }
}
