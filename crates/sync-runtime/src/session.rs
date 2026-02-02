use crate::guest::{
    decode_payload_base64, encode_payload_base64, GuestAction, GuestContext, GuestContextRole,
    GuestError, GuestErrorCode, GuestMode, GuestPermission, GuestRequest, GuestResponse,
    GUEST_PROTOCOL_VERSION,
};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use sync_format::{ManifestPermissions, SyncArchive};

static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

/// A guest session for executing operations on a `.sync` archive.
#[derive(Debug, Clone)]
pub struct GuestSession {
    /// Path to the `.sync` archive.
    pub sync_path: PathBuf,
    /// Execution mode.
    pub mode: GuestMode,
    /// Session role.
    pub role: GuestContextRole,
    /// Session permissions.
    pub permissions: GuestPermission,
    /// Manifest-defined permissions.
    pub manifest_permissions: ManifestPermissions,
    /// Optional host application identifier.
    pub host_app: Option<String>,
    /// CPU time limit in milliseconds.
    pub cpu_limit_ms: Option<u64>,
    /// Memory limit in megabytes.
    pub memory_limit_mb: Option<u64>,
    /// Widget bounds for UI mode.
    pub widget_bounds: Option<WidgetBounds>,
}

/// Bounds for widget rendering.
#[derive(Debug, Clone, Copy)]
pub struct WidgetBounds {
    /// X coordinate.
    pub x: u32,
    /// Y coordinate.
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl GuestSession {
    /// Create a new session for the given `.sync` archive.
    pub fn new(sync_path: PathBuf) -> Result<Self, GuestError> {
        let archive = SyncArchive::open(&sync_path).map_err(|e| {
            GuestError::new(
                GuestErrorCode::InvalidRequest,
                format!("Failed to open sync archive: {}", e),
            )
        })?;

        Ok(Self {
            sync_path,
            mode: GuestMode::Widget,
            role: GuestContextRole::Consumer,
            permissions: GuestPermission {
                can_read_payload: true,
                can_read_context: true,
                can_write_payload: false,
                can_write_context: false,
                can_execute_wasm: false,
                allowed_hosts: Vec::new(),
                allowed_env: Vec::new(),
            },
            manifest_permissions: archive.manifest().permissions.clone(),
            host_app: None,
            cpu_limit_ms: None,
            memory_limit_mb: None,
            widget_bounds: None,
        })
    }

    /// Create a new session with a specified host application.
    pub fn new_with_host_app(sync_path: PathBuf, host_app: &str) -> Result<Self, GuestError> {
        let mut session = Self::new(sync_path)?;
        session.set_host_app(host_app)?;
        Ok(session)
    }

    /// Set the host application identifier.
    pub fn set_host_app(&mut self, host_app: &str) -> Result<(), GuestError> {
        if host_app.trim().is_empty() {
            return Err(GuestError::new(
                GuestErrorCode::InvalidRequest,
                "host_app cannot be empty",
            ));
        }
        self.host_app = Some(host_app.to_string());
        Ok(())
    }

    /// Set the session to widget mode.
    pub fn as_widget(&mut self, host_app: &str) -> Result<(), GuestError> {
        self.mode = GuestMode::Widget;
        self.host_app = Some(host_app.to_string());
        Ok(())
    }

    /// Set the session to headless mode.
    pub fn as_headless(&mut self, host_app: &str) -> Result<(), GuestError> {
        self.mode = GuestMode::Headless;
        self.host_app = Some(host_app.to_string());
        Ok(())
    }

    /// Set the role to consumer.
    pub fn as_consumer(&mut self) -> Result<(), GuestError> {
        self.role = GuestContextRole::Consumer;
        Ok(())
    }

    /// Set the role to owner.
    pub fn as_owner(&mut self) -> Result<(), GuestError> {
        self.role = GuestContextRole::Owner;
        Ok(())
    }

    /// Grant permission to read payload.
    pub fn grant_read_payload(&mut self) -> Result<(), GuestError> {
        self.permissions.can_read_payload = true;
        Ok(())
    }

    /// Grant permission to read context.
    pub fn grant_read_context(&mut self) -> Result<(), GuestError> {
        self.permissions.can_read_context = true;
        Ok(())
    }

    /// Grant permission to write payload.
    pub fn grant_write_payload(&mut self) -> Result<(), GuestError> {
        self.permissions.can_write_payload = true;
        Ok(())
    }

    /// Grant permission to write context.
    pub fn grant_context_write(&mut self) -> Result<(), GuestError> {
        self.permissions.can_write_context = true;
        Ok(())
    }

    /// Grant permission to execute WASM.
    pub fn grant_wasm_execution(&mut self) -> Result<(), GuestError> {
        self.permissions.can_execute_wasm = true;
        Ok(())
    }

    /// Revoke permission to execute WASM.
    pub fn revoke_wasm_execution(&mut self) -> Result<(), GuestError> {
        self.permissions.can_execute_wasm = false;
        Ok(())
    }

    /// Add a host to the allowed hosts list.
    pub fn allow_host(&mut self, host: &str) -> Result<(), GuestError> {
        if !self.permissions.allowed_hosts.iter().any(|h| h == host) {
            self.permissions.allowed_hosts.push(host.to_string());
        }
        Ok(())
    }

    /// Add an environment variable to the allowed list.
    pub fn allow_env_var(&mut self, var: &str) -> Result<(), GuestError> {
        if !self.permissions.allowed_env.iter().any(|v| v == var) {
            self.permissions.allowed_env.push(var.to_string());
        }
        Ok(())
    }

    /// Set the CPU time limit.
    pub fn set_cpu_limit_ms(&mut self, limit_ms: u64) -> Result<(), GuestError> {
        self.cpu_limit_ms = Some(limit_ms);
        Ok(())
    }

    /// Set the memory limit.
    pub fn set_memory_limit_mb(&mut self, limit_mb: u64) -> Result<(), GuestError> {
        self.memory_limit_mb = Some(limit_mb);
        Ok(())
    }

    /// Set widget bounds for UI mode.
    pub fn set_widget_bounds(&mut self, bounds: WidgetBounds) -> Result<(), GuestError> {
        if bounds.width == 0 || bounds.height == 0 {
            return Err(GuestError::new(
                GuestErrorCode::InvalidRequest,
                "widget bounds must have positive width and height",
            ));
        }
        self.widget_bounds = Some(bounds);
        Ok(())
    }

    /// Execute a read payload action.
    pub fn execute_read_payload(&self) -> Result<GuestResponse, GuestError> {
        self.execute_request(GuestAction::ReadPayload, Value::Null)
    }

    /// Execute a read payload action and return raw bytes.
    pub fn execute_read_payload_bytes(&self) -> Result<Vec<u8>, GuestError> {
        let response = self.execute_read_payload()?;
        if !response.ok {
            return Err(response.error.unwrap_or_else(|| {
                GuestError::new(GuestErrorCode::ExecutionFailed, "Read payload failed")
            }));
        }

        let payload = response
            .result
            .ok_or_else(|| GuestError::new(GuestErrorCode::ExecutionFailed, "Missing payload"))?;

        let payload_str = payload.as_str().ok_or_else(|| {
            GuestError::new(
                GuestErrorCode::InvalidRequest,
                "payload must be a base64 string",
            )
        })?;

        decode_payload_base64(payload_str)
    }

    /// Execute a read context action.
    pub fn execute_read_context(&self) -> Result<GuestResponse, GuestError> {
        self.execute_request(GuestAction::ReadContext, Value::Null)
    }

    /// Execute a write payload action with string content.
    pub fn execute_write_payload(&self, new_content: String) -> Result<GuestResponse, GuestError> {
        self.execute_write_payload_bytes(new_content.as_bytes())
    }

    /// Execute a write payload action with raw bytes.
    pub fn execute_write_payload_bytes(
        &self,
        new_content: &[u8],
    ) -> Result<GuestResponse, GuestError> {
        let encoded = encode_payload_base64(new_content);
        self.execute_request(GuestAction::WritePayload, Value::String(encoded))
    }

    /// Execute an update payload action with string content.
    pub fn execute_update_payload(&self, new_content: String) -> Result<GuestResponse, GuestError> {
        self.execute_update_payload_bytes(new_content.as_bytes())
    }

    /// Execute an update payload action with raw bytes.
    pub fn execute_update_payload_bytes(
        &self,
        new_content: &[u8],
    ) -> Result<GuestResponse, GuestError> {
        let encoded = encode_payload_base64(new_content);
        self.execute_request(GuestAction::UpdatePayload, Value::String(encoded))
    }

    /// Execute a write context action.
    pub fn execute_write_context(&self, new_context: Value) -> Result<GuestResponse, GuestError> {
        self.execute_request(GuestAction::WriteContext, new_context)
    }

    /// Execute the WASM module.
    pub fn execute_wasm(&self) -> Result<GuestResponse, GuestError> {
        self.execute_request(GuestAction::ExecuteWasm, Value::Null)
    }

    /// Execute an arbitrary guest request.
    pub fn execute_request(
        &self,
        action: GuestAction,
        input: Value,
    ) -> Result<GuestResponse, GuestError> {
        self.verify_permissions(&action)?;

        let command_path = self.host_app.as_ref().ok_or_else(|| {
            GuestError::new(GuestErrorCode::HostUnavailable, "Host app not configured")
        })?;
        let request = self.build_request(action, input);

        let mut command = Command::new(command_path);
        command
            .args(self.build_guest_command())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        self.apply_env_policy(&mut command, &request)?;

        let mut child = command
            .spawn()
            .map_err(|e| GuestError::new(GuestErrorCode::ExecutionFailed, e.to_string()))?;

        {
            let mut stdin = child.stdin.take().ok_or_else(|| {
                GuestError::new(GuestErrorCode::HostUnavailable, "stdin unavailable")
            })?;
            let request_bytes = serde_json::to_vec(&request)
                .map_err(|e| GuestError::new(GuestErrorCode::InvalidRequest, e.to_string()))?;
            stdin
                .write_all(&request_bytes)
                .map_err(|e| GuestError::new(GuestErrorCode::IoError, e.to_string()))?;
            stdin
                .write_all(b"\n")
                .map_err(|e| GuestError::new(GuestErrorCode::IoError, e.to_string()))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| GuestError::new(GuestErrorCode::ExecutionFailed, e.to_string()))?;

        let mut response = self.parse_response(&request.request_id, &output.stdout, &output.stderr);

        if !output.status.success() && response.ok {
            response.ok = false;
            response.error = Some(GuestError::new(
                GuestErrorCode::ExecutionFailed,
                format!("Guest exited with status {}", output.status),
            ));
        }

        if !response.ok && response.error.is_none() {
            response.error = Some(GuestError::new(
                GuestErrorCode::ExecutionFailed,
                "Guest failed without error payload",
            ));
        }

        Ok(response)
    }

    fn build_request(&self, action: GuestAction, input: Value) -> GuestRequest {
        GuestRequest {
            version: GUEST_PROTOCOL_VERSION.to_string(),
            request_id: self.next_request_id(),
            action,
            context: self.get_context(),
            input,
        }
    }

    fn build_guest_command(&self) -> Vec<String> {
        vec![
            "guest".to_string(),
            self.sync_path.to_string_lossy().to_string(),
        ]
    }

    fn apply_env_policy(
        &self,
        command: &mut Command,
        request: &GuestRequest,
    ) -> Result<(), GuestError> {
        let effective_permissions = self.effective_permissions();

        match (self.mode.clone(), self.widget_bounds) {
            (GuestMode::Widget, None) => {
                return Err(GuestError::new(
                    GuestErrorCode::InvalidRequest,
                    "widget bounds are required for widget mode",
                ));
            }
            (GuestMode::Headless, Some(_)) => {
                return Err(GuestError::new(
                    GuestErrorCode::InvalidRequest,
                    "widget bounds are not allowed for headless mode",
                ));
            }
            _ => {}
        }

        command.env_clear();
        Self::apply_baseline_env(command);

        for env_var in &effective_permissions.allowed_env {
            if let Ok(value) = std::env::var(env_var) {
                command.env(env_var, value);
            }
        }

        command.env("ALLOW_HOSTS", effective_permissions.allowed_hosts.join(","));
        command.env("ALLOW_ENV", effective_permissions.allowed_env.join(","));
        command.env("CAPSULE_GUEST_PROTOCOL", &request.version);
        command.env("SYNC_PATH", self.sync_path.to_string_lossy().to_string());
        command.env(
            "GUEST_MODE",
            match self.mode {
                GuestMode::Widget => "widget",
                GuestMode::Headless => "headless",
            },
        );
        command.env(
            "GUEST_ROLE",
            match self.role {
                GuestContextRole::Consumer => "consumer",
                GuestContextRole::Owner => "owner",
            },
        );

        if let Some(limit_ms) = self.cpu_limit_ms {
            command.env("GUEST_CPU_LIMIT_MS", limit_ms.to_string());
        }

        if let Some(limit_mb) = self.memory_limit_mb {
            command.env("GUEST_MEMORY_LIMIT_MB", limit_mb.to_string());
        }

        if let (GuestMode::Widget, Some(bounds)) = (self.mode.clone(), self.widget_bounds) {
            let value = format!(
                "{},{},{},{}",
                bounds.x, bounds.y, bounds.width, bounds.height
            );
            command.env("GUEST_WIDGET_BOUNDS", value);
        }

        Ok(())
    }

    fn apply_baseline_env(command: &mut Command) {
        for key in ["PATH", "LANG", "LC_ALL", "HOME", "USER"] {
            if let Ok(value) = std::env::var(key) {
                command.env(key, value);
            }
        }

        for (key, value) in std::env::vars() {
            if key.starts_with("CAPSULE_") || key.starts_with("ATO_") {
                command.env(key, value);
            }
        }
    }

    fn parse_response(&self, request_id: &str, stdout: &[u8], stderr: &[u8]) -> GuestResponse {
        let parsed = serde_json::from_slice::<GuestResponse>(stdout).or_else(|err| {
            if stderr.is_empty() {
                Err(err)
            } else {
                serde_json::from_slice::<GuestResponse>(stderr)
            }
        });

        match parsed {
            Ok(response) => {
                if response.request_id != request_id {
                    return self.protocol_error(request_id, "request_id mismatch", stdout, stderr);
                }

                if response.version != GUEST_PROTOCOL_VERSION {
                    return self.protocol_error(
                        request_id,
                        "protocol version mismatch",
                        stdout,
                        stderr,
                    );
                }

                response
            }
            Err(err) => self.protocol_error(request_id, err.to_string(), stdout, stderr),
        }
    }

    fn protocol_error(
        &self,
        request_id: &str,
        message: impl Into<String>,
        stdout: &[u8],
        stderr: &[u8],
    ) -> GuestResponse {
        GuestResponse {
            version: GUEST_PROTOCOL_VERSION.to_string(),
            request_id: request_id.to_string(),
            ok: false,
            result: Some(json!({
                "stdout": String::from_utf8_lossy(stdout).to_string(),
                "stderr": String::from_utf8_lossy(stderr).to_string(),
            })),
            error: Some(GuestError::new(GuestErrorCode::ProtocolError, message)),
        }
    }

    /// Verify that the current session has permission for the given action.
    pub fn verify_permissions(&self, action: &GuestAction) -> Result<(), GuestError> {
        if matches!(self.role, GuestContextRole::Consumer) {
            match action {
                GuestAction::ReadPayload | GuestAction::ReadContext => {}
                _ => {
                    return Err(GuestError::new(
                        GuestErrorCode::PermissionDenied,
                        "Owner context required",
                    ));
                }
            }
        }

        let effective_permissions = self.effective_permissions();

        match action {
            GuestAction::ReadPayload => {
                if !effective_permissions.can_read_payload {
                    return Err(GuestError::new(
                        GuestErrorCode::PermissionDenied,
                        "read payload not allowed",
                    ));
                }
            }
            GuestAction::ReadContext => {
                if !effective_permissions.can_read_context {
                    return Err(GuestError::new(
                        GuestErrorCode::PermissionDenied,
                        "read context not allowed",
                    ));
                }
            }
            GuestAction::WritePayload | GuestAction::UpdatePayload => {
                if !effective_permissions.can_write_payload {
                    return Err(GuestError::new(
                        GuestErrorCode::PermissionDenied,
                        "write payload not allowed",
                    ));
                }
            }
            GuestAction::WriteContext => {
                if !effective_permissions.can_write_context {
                    return Err(GuestError::new(
                        GuestErrorCode::PermissionDenied,
                        "write context not allowed",
                    ));
                }
            }
            GuestAction::ExecuteWasm => {
                if !effective_permissions.can_execute_wasm {
                    return Err(GuestError::new(
                        GuestErrorCode::PermissionDenied,
                        "execute wasm not allowed",
                    ));
                }
            }
        }

        Ok(())
    }

    /// Get the current context for guest operations.
    pub fn get_context(&self) -> GuestContext {
        GuestContext {
            mode: self.mode.clone(),
            role: self.role.clone(),
            permissions: self.effective_permissions(),
            sync_path: self.sync_path.to_string_lossy().to_string(),
            host_app: self.host_app.clone(),
        }
    }

    fn effective_permissions(&self) -> GuestPermission {
        let mut permissions = self.permissions.clone();
        permissions.allowed_env = intersect_allowlist(
            &self.permissions.allowed_env,
            &self.manifest_permissions.allow_env,
        );
        permissions.allowed_hosts = intersect_allowlist(
            &self.permissions.allowed_hosts,
            &self.manifest_permissions.allow_hosts,
        );
        permissions
    }

    fn next_request_id(&self) -> String {
        let seq = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("guest-{}", seq)
    }
}

fn intersect_allowlist(host: &[String], manifest: &[String]) -> Vec<String> {
    if host.is_empty() || manifest.is_empty() {
        return Vec::new();
    }

    let manifest_set: HashSet<&str> = manifest.iter().map(String::as_str).collect();
    host.iter()
        .filter(|item| manifest_set.contains(item.as_str()))
        .cloned()
        .collect()
}
