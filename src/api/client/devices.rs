//! `ApiClient` endpoint methods for the Devices API group
//! (`/pay-api/v1/devices/*`).
//!
//! Each method calls `self.send(…)` from the core in `mod.rs`. The bodyless
//! POST (`ttp_activate`) uses `send_no_body(POST, path)` so it still sends
//! Content-Length: 0 as the Flute API requires, while tolerating the empty
//! HTTP 200 body the activate endpoint returns.

use crate::api::client::ApiClient;
use crate::api::error::ApiError;
use reqwest::Method;

impl ApiClient {
    /// GET `/pay-api/v1/devices` — list all devices.
    ///
    /// Response: `GetIsvDevicesResponseDto` `{devices: [DeviceResponseDto]}`.
    pub async fn list_devices(&self) -> Result<serde_json::Value, ApiError> {
        self.send(Method::GET, "/pay-api/v1/devices", None).await
    }

    /// GET `/pay-api/v1/devices/{id}` — fetch a single device by ID.
    pub async fn get_device(&self, id: &str) -> Result<serde_json::Value, ApiError> {
        let path = format!("/pay-api/v1/devices/{id}");
        self.send(Method::GET, &path, None).await
    }

    /// POST `/pay-api/v1/devices` — register (create or update) a device.
    ///
    /// Body: `CreateOrUpdateIsvDeviceRequestDto` `{deviceId, deviceName?}`.
    pub async fn register_device(
        &self,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
        self.send(Method::POST, "/pay-api/v1/devices", Some(body))
            .await
    }

    /// POST `/pay-api/v1/devices/tap-to-pay/jwt` — generate a Tap-to-Pay JWT.
    ///
    /// Body: `GenerateIsvTapToPayJwtRequestDto` `{deviceId}`.
    pub async fn ttp_jwt(&self, body: serde_json::Value) -> Result<serde_json::Value, ApiError> {
        self.send(
            Method::POST,
            "/pay-api/v1/devices/tap-to-pay/jwt",
            Some(body),
        )
        .await
    }

    /// POST `/pay-api/v1/devices/{id}/tap-to-pay/activate` — activate Tap-to-Pay.
    ///
    /// Bodyless request. The live API responds HTTP 200 with an **empty** body,
    /// so this uses `send_no_body` (which discards the success body) rather than
    /// `send::<Value>` — otherwise an empty 200 fails to decode ("EOF while
    /// parsing a value"). Callers that need the post-activation device state
    /// should issue a subsequent `get_device`. (ARISE-4505 BUG-09.)
    pub async fn ttp_activate(&self, id: &str) -> Result<(), ApiError> {
        let path = format!("/pay-api/v1/devices/{id}/tap-to-pay/activate");
        self.send_no_body(Method::POST, &path).await
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::api::client::test_client;
    use crate::cli::devices::{build_register_device_body, build_ttp_jwt_body};
    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, header, method, path},
    };

    fn sample_device() -> serde_json::Value {
        json!({
            "id": "dev-001",
            "deviceId": "DEVICE-ABC-123",
            "deviceName": "Register 1",
            "status": "Active"
        })
    }

    fn sample_devices_list() -> serde_json::Value {
        json!({
            "devices": [sample_device()]
        })
    }

    // ── list_devices ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_devices_hits_correct_path() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pay-api/v1/devices"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_devices_list()))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.list_devices().await.unwrap();
        assert!(result["devices"].as_array().is_some());
        assert_eq!(result["devices"].as_array().unwrap().len(), 1);
    }

    // ── get_device ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn get_device_hits_correct_path() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/pay-api/v1/devices/dev-001"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_device()))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.get_device("dev-001").await.unwrap();
        assert_eq!(result["id"], "dev-001");
    }

    // ── register_device ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn register_device_posts_body_to_correct_path() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/devices"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(json!({"deviceId": "DEVICE-ABC-123"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_device()))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let body = build_register_device_body("DEVICE-ABC-123", Some("Register 1"));
        let result = api.register_device(body).await.unwrap();
        assert_eq!(result["id"], "dev-001");
    }

    // ── ttp_jwt ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn ttp_jwt_posts_body_to_correct_path() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/devices/tap-to-pay/jwt"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(json!({"deviceId": "DEVICE-ABC-123"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "jwt": "eyJhbGciOiJSUzI1NiJ9.test.signature",
                "deviceId": "DEVICE-ABC-123"
            })))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let body = build_ttp_jwt_body("DEVICE-ABC-123");
        let result = api.ttp_jwt(body).await.unwrap();
        assert!(result["jwt"].as_str().is_some());
    }

    // ── ttp_activate (bodyless POST) ──────────────────────────────────────────

    #[tokio::test]
    async fn ttp_activate_posts_bodyless_to_correct_path() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/devices/dev-001/tap-to-pay/activate"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.ttp_activate("dev-001").await;
        assert!(result.is_ok(), "activate should succeed, got: {result:?}");

        // Verify no body was sent (Content-Length: 0 only)
        let received = server.received_requests().await.unwrap();
        assert_eq!(received.len(), 1);
        assert!(received[0].body.is_empty());
    }

    /// Regression (ARISE-4505 BUG-09): the live activate endpoint responds with
    /// HTTP 200 and an EMPTY body. `send::<Value>` previously failed to decode it
    /// ("EOF while parsing a value at line 1 column 0") even though activation
    /// succeeded. `ttp_activate` must tolerate an empty 200 the same way
    /// `update_customer` does.
    #[tokio::test]
    async fn ttp_activate_tolerates_empty_200_body() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/pay-api/v1/devices/dev-001/tap-to-pay/activate"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200)) // empty body — mirrors live API
            .mount(&server)
            .await;

        let api = test_client(server.uri());
        let result = api.ttp_activate("dev-001").await;
        assert!(
            result.is_ok(),
            "ttp_activate must succeed on an empty 200 body, got: {result:?}"
        );
    }
}
