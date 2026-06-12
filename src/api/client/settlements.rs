//! `ApiClient` endpoint methods for the Settlements API group
//! (`/pay-api/v1/settlements/*`).

use crate::api::client::ApiClient;
use crate::api::error::ApiError;
use reqwest::Method;
use url::form_urlencoded;

impl ApiClient {
    /// GET `/pay-api/v1/settlements/batches` — list settlement batches.
    ///
    /// Only query parameters that are `Some` are appended to the URL.
    ///
    /// - `page`      → `page` query param
    /// - `page_size` → `pageSize` query param
    /// - `date_from` → `dateFrom` query param
    /// - `date_to`   → `dateTo` query param
    /// - `status_id` → `statusId` query param (1=Open, 2=Settled)
    pub async fn list_settlement_batches(
        &self,
        page: Option<u32>,
        page_size: Option<u32>,
        date_from: Option<&str>,
        date_to: Option<&str>,
        status_id: Option<u32>,
    ) -> Result<serde_json::Value, ApiError> {
        let mut serializer = form_urlencoded::Serializer::new(String::new());
        if let Some(p) = page {
            serializer.append_pair("page", &p.to_string());
        }
        if let Some(ps) = page_size {
            serializer.append_pair("pageSize", &ps.to_string());
        }
        if let Some(df) = date_from {
            serializer.append_pair("dateFrom", df);
        }
        if let Some(dt) = date_to {
            serializer.append_pair("dateTo", dt);
        }
        if let Some(sid) = status_id {
            serializer.append_pair("statusId", &sid.to_string());
        }
        let qs = serializer.finish();

        let path = if qs.is_empty() {
            "/pay-api/v1/settlements/batches".to_string()
        } else {
            format!("/pay-api/v1/settlements/batches?{qs}")
        };

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

    fn sample_batch() -> serde_json::Value {
        json!({
            "id": "batch-001",
            "paymentProcessorName": "TSYS",
            "externalBatchId": "EXT-001",
            "batchDateTime": "2024-03-15T10:00:00Z",
            "transactionCount": 42,
            "salesAmount": 1500.00,
            "refundsAmount": 50.00,
            "netAmount": 1450.00,
            "statusId": 2,
            "statusName": "Settled"
        })
    }

    // ── list_settlement_batches ───────────────────────────────────────────────

    #[tokio::test]
    async fn list_settlement_batches_no_params_hits_base_path() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pay-api/v1/settlements/batches"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [sample_batch()],
                "total": 1
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api
            .list_settlement_batches(None, None, None, None, None)
            .await
            .unwrap();
        assert_eq!(result["total"], 1);
        assert_eq!(result["items"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn list_settlement_batches_passes_page_size_query_param() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pay-api/v1/settlements/batches"))
            .and(query_param("pageSize", "100"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [],
                "total": 0
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api
            .list_settlement_batches(None, Some(100), None, None, None)
            .await
            .unwrap();
        assert_eq!(result["total"], 0);
    }

    #[tokio::test]
    async fn list_settlement_batches_passes_date_range_query_params() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pay-api/v1/settlements/batches"))
            .and(query_param("dateFrom", "2024-01-01"))
            .and(query_param("dateTo", "2024-03-31"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [sample_batch()],
                "total": 1
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api
            .list_settlement_batches(None, None, Some("2024-01-01"), Some("2024-03-31"), None)
            .await
            .unwrap();
        assert_eq!(result["total"], 1);
    }

    #[tokio::test]
    async fn list_settlement_batches_passes_status_id_query_param() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pay-api/v1/settlements/batches"))
            .and(query_param("statusId", "2"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [sample_batch()],
                "total": 1
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api
            .list_settlement_batches(None, None, None, None, Some(2))
            .await
            .unwrap();
        assert_eq!(result["total"], 1);
    }
}
