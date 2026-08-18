//! Tolerant JSON-RPC-like envelopes used by Codex App Server.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::{AppError, AppErrorKind, RecoveryAction};

/// Frozen V1 initialize parameters. `experimentalApi` is deliberately false
/// and is serialized as part of the typed handshake rather than assembled by
/// a caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub client_info: InitializeClientInfo,
    pub capabilities: InitializeCapabilities,
    pub experimental_api: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct InitializeCapabilities {}

impl InitializeParams {
    pub fn v1() -> Self {
        Self {
            client_info: InitializeClientInfo {
                name: "codex-barbar".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: InitializeCapabilities::default(),
            experimental_api: false,
        }
    }
}

/// Numeric correlation identifier used by the app-server JSON-RPC envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RpcId(pub u64);

/// Error body returned for a correlated RPC response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcErrorBody {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Tolerantly classified incoming messages.
#[derive(Debug, Clone, PartialEq)]
pub enum IncomingMessage {
    Response {
        id: RpcId,
        result: Value,
    },
    Error {
        id: RpcId,
        error: RpcErrorBody,
    },
    Notification {
        method: String,
        params: Value,
    },
    ServerRequest {
        id: RpcId,
        method: String,
        params: Value,
    },
}

fn protocol_error(code: &'static str) -> AppError {
    AppError::new(
        AppErrorKind::ProtocolMismatch,
        "errors.appServerProtocolMismatch",
        RecoveryAction::InstallTestedCodex,
        code,
    )
}

fn parse_id(value: &Value) -> Result<RpcId, AppError> {
    value
        .as_u64()
        .map(RpcId)
        .ok_or_else(|| protocol_error("APP_SERVER_INVALID_RPC_ID"))
}

/// Parse one JSON object into a tolerant incoming message classification.
pub fn parse_incoming(bytes: &[u8]) -> Result<IncomingMessage, AppError> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| protocol_error("APP_SERVER_INVALID_JSON"))?;
    parse_incoming_value(value)
}

/// Classify an already-decoded JSON value.
///
/// The bounded codec performs UTF-8 and JSON validation while reading a
/// frame. Keeping this second stage separate lets the client avoid reparsing
/// a valid frame just to classify its JSON-RPC envelope.
pub fn parse_incoming_value(value: Value) -> Result<IncomingMessage, AppError> {
    let mut object = value
        .as_object()
        .cloned()
        .ok_or_else(|| protocol_error("APP_SERVER_REQUIRED_FIELD_MISSING"))?;
    let id = object.remove("id");
    let method = object.remove("method");
    let params = object.remove("params").unwrap_or(Value::Null);

    if let Some(id_value) = id {
        let id = parse_id(&id_value)?;
        if let Some(error_value) = object.remove("error") {
            let error = serde_json::from_value(error_value)
                .map_err(|_| protocol_error("APP_SERVER_INVALID_ERROR_BODY"))?;
            return Ok(IncomingMessage::Error { id, error });
        }
        if let Some(method_value) = method {
            let method = method_value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| protocol_error("APP_SERVER_INVALID_METHOD"))?;
            return Ok(IncomingMessage::ServerRequest { id, method, params });
        }
        if let Some(result) = object.remove("result") {
            return Ok(IncomingMessage::Response { id, result });
        }
        return Err(protocol_error("APP_SERVER_REQUIRED_FIELD_MISSING"));
    }

    if let Some(method_value) = method {
        let method = method_value
            .as_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| protocol_error("APP_SERVER_INVALID_METHOD"))?;
        return Ok(IncomingMessage::Notification { method, params });
    }

    Err(protocol_error("APP_SERVER_REQUIRED_FIELD_MISSING"))
}

#[derive(Serialize)]
struct RequestEnvelope<'a> {
    id: RpcId,
    method: &'a str,
    params: Value,
}

#[derive(Serialize)]
struct NotificationEnvelope<'a> {
    method: &'a str,
    params: Value,
}

/// Encode one request frame, including its trailing LF.
pub fn encode_request(id: RpcId, method: &str, params: Value) -> Result<Vec<u8>, AppError> {
    encode_frame(&RequestEnvelope { id, method, params })
}

/// Encode one notification frame, including its trailing LF.
pub fn encode_notification(method: &str, params: Value) -> Result<Vec<u8>, AppError> {
    encode_frame(&NotificationEnvelope { method, params })
}

fn encode_frame<T: Serialize>(value: &T) -> Result<Vec<u8>, AppError> {
    let mut bytes =
        serde_json::to_vec(value).map_err(|_| protocol_error("APP_SERVER_ENCODE_FAILED"))?;
    if bytes.len() > crate::providers::codex::app_server::codec::MAX_JSONL_BYTES {
        return Err(protocol_error("APP_SERVER_LINE_TOO_LARGE"));
    }
    bytes.push(b'\n');
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_ignores_unknown_fields_but_requires_id() {
        let incoming = parse_incoming(br#"{"id":7,"result":{},"futureField":true}"#).unwrap();
        assert!(matches!(
            incoming,
            IncomingMessage::Response { id: RpcId(7), .. }
        ));
        assert_eq!(
            parse_incoming(br#"{"result":{}}"#)
                .unwrap_err()
                .diagnostic_code,
            "APP_SERVER_REQUIRED_FIELD_MISSING"
        );
    }

    #[test]
    fn classifies_error_notification_and_server_request() {
        let error =
            parse_incoming(br#"{"id":3,"error":{"code":-1,"message":"nope"},"futureField":true}"#)
                .unwrap();
        assert!(matches!(error, IncomingMessage::Error { id: RpcId(3), .. }));

        let notification = parse_incoming(br#"{"method":"initialized","params":{}}"#).unwrap();
        assert!(matches!(
            notification,
            IncomingMessage::Notification { method, .. } if method == "initialized"
        ));

        let request = parse_incoming(br#"{"id":9,"method":"server/ping","params":{}}"#).unwrap();
        assert!(matches!(
            request,
            IncomingMessage::ServerRequest { id: RpcId(9), method, .. } if method == "server/ping"
        ));
    }

    #[test]
    fn rejects_unknown_shape_and_non_numeric_id() {
        let error = parse_incoming(br#"{"foo":true}"#).unwrap_err();
        assert_eq!(error.diagnostic_code, "APP_SERVER_REQUIRED_FIELD_MISSING");

        let error = parse_incoming(br#"{"id":"7","result":{}}"#).unwrap_err();
        assert_eq!(error.diagnostic_code, "APP_SERVER_INVALID_RPC_ID");
    }

    #[test]
    fn encodes_request_with_fixed_method_and_params() {
        let encoded = encode_request(
            RpcId(4),
            "account/read",
            serde_json::json!({"refreshToken": false}),
        )
        .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(value["id"], 4);
        assert_eq!(value["method"], "account/read");
        assert_eq!(value["params"]["refreshToken"], false);
        assert!(encoded.ends_with(b"\n"));
    }

    #[test]
    fn encodes_notification_without_id() {
        let encoded = encode_notification("initialized", serde_json::json!({})).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        assert!(value.get("id").is_none());
        assert_eq!(value["method"], "initialized");
    }
}
