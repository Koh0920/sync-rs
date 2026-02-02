use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};

/// Protocol version for guest communication.
pub const GUEST_PROTOCOL_VERSION: &str = "guest.v1";

/// Execution mode for guest sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GuestMode {
    /// Widget mode with UI bounds.
    Widget,
    /// Headless mode without UI.
    Headless,
}

/// Role of the session initiator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GuestContextRole {
    /// Consumer with read-only access.
    Consumer,
    /// Owner with full access.
    Owner,
}

/// Permission set for guest operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuestPermission {
    /// Whether payload reading is allowed.
    #[serde(default)]
    pub can_read_payload: bool,
    /// Whether context reading is allowed.
    #[serde(default)]
    pub can_read_context: bool,
    /// Whether payload writing is allowed.
    #[serde(default)]
    pub can_write_payload: bool,
    /// Whether context writing is allowed.
    #[serde(default)]
    pub can_write_context: bool,
    /// Whether WASM execution is allowed.
    #[serde(default)]
    pub can_execute_wasm: bool,
    /// List of allowed network hosts.
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
    /// List of allowed environment variables.
    #[serde(default)]
    pub allowed_env: Vec<String>,
}

/// Context information passed to guest modules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestContext {
    /// Execution mode.
    pub mode: GuestMode,
    /// Session role.
    pub role: GuestContextRole,
    /// Effective permissions.
    pub permissions: GuestPermission,
    /// Path to the `.sync` archive.
    pub sync_path: String,
    /// Optional host application identifier.
    pub host_app: Option<String>,
}

/// Request structure for guest operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestRequest {
    /// Protocol version.
    pub version: String,
    /// Unique request identifier.
    pub request_id: String,
    /// Requested action.
    pub action: GuestAction,
    /// Session context.
    pub context: GuestContext,
    /// Optional input data.
    #[serde(default)]
    pub input: serde_json::Value,
}

/// Available guest actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GuestAction {
    /// Read the payload content.
    ReadPayload,
    /// Read the context data.
    ReadContext,
    /// Write new payload content.
    WritePayload,
    /// Write new context data.
    WriteContext,
    /// Execute the WASM module.
    ExecuteWasm,
    /// Update the payload in place.
    UpdatePayload,
}

/// Response structure from guest operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestResponse {
    /// Protocol version.
    pub version: String,
    /// Matching request identifier.
    pub request_id: String,
    /// Whether the operation succeeded.
    pub ok: bool,
    /// Optional result data.
    pub result: Option<serde_json::Value>,
    /// Optional error information.
    pub error: Option<GuestError>,
}

/// Error codes for guest operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GuestErrorCode {
    /// Operation not permitted.
    PermissionDenied,
    /// Malformed request.
    InvalidRequest,
    /// Execution failed.
    ExecutionFailed,
    /// Host application unavailable.
    HostUnavailable,
    /// Protocol violation.
    ProtocolError,
    /// I/O error.
    IoError,
}

/// Error information from guest operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestError {
    /// Error code.
    pub code: GuestErrorCode,
    /// Human-readable message.
    pub message: String,
}

impl GuestError {
    /// Create a new guest error.
    pub fn new(code: GuestErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Encode payload bytes as base64.
pub fn encode_payload_base64(payload: &[u8]) -> String {
    general_purpose::STANDARD.encode(payload)
}

/// Decode base64 payload to bytes.
pub fn decode_payload_base64(value: &str) -> Result<Vec<u8>, GuestError> {
    general_purpose::STANDARD
        .decode(value)
        .map_err(|err| GuestError::new(GuestErrorCode::InvalidRequest, err.to_string()))
}
