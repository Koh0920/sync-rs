use crate::host::{HostFunctions, HostState};
use crate::{Error, ExecutionResult, Result};
use secrecy::SecretString;
use std::io::{Read, Seek, SeekFrom};
use sync_format::{SyncArchive, SyncVariant};
use wasmtime::{Engine, Linker, Module, Store};

/// Configuration for WASM execution.
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// Execution timeout in seconds.
    pub timeout_secs: u64,
    /// Whether HTTP host functions are enabled.
    pub enable_http: bool,
    /// Whether payload I/O host functions are enabled.
    pub enable_payload_io: bool,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            enable_http: true,
            enable_payload_io: true,
        }
    }
}

/// WASM runtime runner using wasmtime.
pub struct WasmRunner {
    engine: Engine,
    config: ExecutionConfig,
    password: Option<SecretString>,
    decrypted_payload: Option<Vec<u8>>,
}

impl WasmRunner {
    /// Create a new `WasmRunner` with default configuration.
    pub fn new() -> Result<Self> {
        let engine = Engine::default();
        Ok(Self {
            engine,
            config: ExecutionConfig::default(),
            password: None,
            decrypted_payload: None,
        })
    }

    /// Set the execution configuration.
    pub fn with_config(mut self, config: ExecutionConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the password for vault (encrypted) archives.
    pub fn with_password(mut self, password: SecretString) -> Self {
        self.password = Some(password);
        self
    }

    /// Execute a WASM module from a sync archive.
    pub fn execute(
        &mut self,
        archive: &mut SyncArchive,
        action: &str,
        _input: Option<serde_json::Value>,
    ) -> Result<ExecutionResult> {
        let payload_data = self.read_payload_data(archive)?;

        let is_vault = archive.manifest().sync.variant == SyncVariant::Vault;
        if is_vault && self.password.is_none() {
            return Err(Error::InvalidInput(
                "Vault archive requires password".to_string(),
            ));
        }

        if is_vault {
            self.decrypted_payload = Some(payload_data.clone());
        }

        let wasm_bytes = self.read_wasm_data(archive)?;

        let sync_path = archive.archive_path().to_string();
        let timeout_secs = self.config.timeout_secs;
        let allowed_hosts = archive.manifest().permissions.allow_hosts.clone();
        let host_state = HostState::new(sync_path, allowed_hosts, timeout_secs);

        {
            let mut buffer = host_state.payload_buffer.lock().unwrap();
            *buffer = payload_data;
        }

        let mut store = Store::new(&self.engine, host_state);
        let mut linker = Linker::new(&self.engine);

        HostFunctions::register(&mut linker)?;

        let module =
            Module::new(&self.engine, &wasm_bytes).map_err(|e| Error::Wasm(e.to_string()))?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| Error::Wasm(e.to_string()))?;

        let result = if let Ok(run) = instance.get_typed_func::<(), ()>(&mut store, "run") {
            run.call(&mut store, ())
                .map_err(|e| Error::Wasm(e.to_string()))?;

            let updated = {
                let state = store.data();
                state.updated_payload.lock().unwrap().clone()
            };

            let payload_updated = if let Some(new_payload) = updated {
                if is_vault {
                    self.decrypted_payload = Some(new_payload.clone());
                }
                archive.update_payload(&new_payload)?;
                true
            } else {
                false
            };

            ExecutionResult {
                success: true,
                result: Some(serde_json::json!({"action": action})),
                error: None,
                payload_updated,
            }
        } else if let Ok(main) = instance.get_typed_func::<(), ()>(&mut store, "_start") {
            main.call(&mut store, ())
                .map_err(|e| Error::Wasm(e.to_string()))?;

            ExecutionResult {
                success: true,
                result: Some(serde_json::json!({"action": action})),
                error: None,
                payload_updated: false,
            }
        } else {
            return Err(Error::Wasm("No entry point found".to_string()));
        };

        Ok(result)
    }

    /// Read payload data from archive.
    fn read_payload_data(&self, archive: &SyncArchive) -> Result<Vec<u8>> {
        let entry = archive
            .payload_entry()
            .ok_or_else(|| Error::InvalidInput("No payload found".to_string()))?;

        let mut file = std::fs::File::open(archive.archive_path())?;
        file.seek(SeekFrom::Start(entry.offset))?;

        let mut data = vec![0u8; entry.size as usize];
        file.read_exact(&mut data)?;

        Ok(data)
    }

    /// Read WASM data from archive.
    fn read_wasm_data(&self, archive: &SyncArchive) -> Result<Vec<u8>> {
        let entry = archive
            .entry("sync.wasm")
            .ok_or_else(|| Error::InvalidInput("No sync.wasm found".to_string()))?;

        let mut file = std::fs::File::open(archive.archive_path())?;
        file.seek(SeekFrom::Start(entry.offset))?;

        let mut data = vec![0u8; entry.size as usize];
        file.read_exact(&mut data)?;

        Ok(data)
    }

    /// Get the decrypted payload (if available).
    pub fn get_decrypted_payload(&self) -> Option<&Vec<u8>> {
        self.decrypted_payload.as_ref()
    }
}

/// Generate a simple request ID.
fn _generate_request_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("req-{}-{}", timestamp, std::process::id())
}
