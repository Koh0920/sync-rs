use crate::{Error, Result};
use std::sync::{Arc, Mutex};
use wasmtime::{Caller, Linker, Memory};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder};

/// State shared between host and WASM guest.
pub struct HostState {
    /// Path to the sync archive.
    pub sync_path: String,
    /// Payload data buffer for zero-copy operations.
    pub payload_buffer: Arc<Mutex<Vec<u8>>>,
    /// Updated payload to be written back.
    pub updated_payload: Arc<Mutex<Option<Vec<u8>>>>,
    /// Last HTTP response buffer.
    pub last_response: Arc<Mutex<Vec<u8>>>,
    /// Allowed hosts from manifest.
    pub allowed_hosts: Vec<String>,
    /// Execution timeout in seconds.
    pub timeout_secs: u64,
    /// WASI context.
    pub wasi: WasiCtx,
}

impl HostState {
    /// Create a new `HostState`.
    pub fn new(sync_path: String, allowed_hosts: Vec<String>, timeout_secs: u64) -> Self {
        Self {
            sync_path,
            payload_buffer: Arc::new(Mutex::new(Vec::new())),
            updated_payload: Arc::new(Mutex::new(None)),
            last_response: Arc::new(Mutex::new(Vec::new())),
            allowed_hosts,
            timeout_secs,
            wasi: WasiCtxBuilder::new().inherit_stdio().build(),
        }
    }

    /// Check if a host is allowed.
    pub fn is_host_allowed(&self, url: &str) -> bool {
        if self.allowed_hosts.is_empty() {
            return false;
        }

        // Extract host from URL (strip protocol and port)
        let host = if let Some(pos) = url.find("://") {
            let rest = &url[pos + 3..];
            let host = if let Some(slash_pos) = rest.find('/') {
                &rest[..slash_pos]
            } else {
                rest
            };
            if let Some(colon_pos) = host.find(':') {
                &host[..colon_pos]
            } else {
                host
            }
        } else {
            url
        };

        self.allowed_hosts.iter().any(|allowed| {
            allowed == host || (allowed.starts_with("*.") && host.ends_with(&allowed[2..]))
        })
    }
}

/// Host functions exposed to WASM guests.
pub struct HostFunctions;

impl HostFunctions {
    /// Register all host functions with the linker.
    pub fn register(linker: &mut Linker<HostState>) -> Result<()> {
        // HTTP request function
        linker
            .func_wrap(
                "host",
                "http_request",
                |mut caller: Caller<'_, HostState>,
                 url_ptr: i32,
                 url_len: i32,
                 method_ptr: i32,
                 method_len: i32|
                 -> i32 {
                    Self::http_request(&mut caller, url_ptr, url_len, method_ptr, method_len)
                },
            )
            .map_err(|e| Error::Wasm(e.to_string()))?;

        // Get last response size
        linker
            .func_wrap(
                "host",
                "last_response_size",
                |caller: Caller<'_, HostState>| -> i32 { Self::last_response_size(&caller) },
            )
            .map_err(|e| Error::Wasm(e.to_string()))?;

        // Read last response
        linker
            .func_wrap(
                "host",
                "last_response_read",
                |mut caller: Caller<'_, HostState>, out_ptr: i32, max_len: i32| -> i32 {
                    Self::last_response_read(&mut caller, out_ptr, max_len)
                },
            )
            .map_err(|e| Error::Wasm(e.to_string()))?;

        // Get payload size
        linker
            .func_wrap(
                "host",
                "payload_size",
                |caller: Caller<'_, HostState>| -> i32 { Self::payload_size(&caller) },
            )
            .map_err(|e| Error::Wasm(e.to_string()))?;

        // Payload read function
        linker
            .func_wrap(
                "host",
                "payload_read",
                |mut caller: Caller<'_, HostState>, offset: i32, len: i32, out_ptr: i32| -> i32 {
                    Self::payload_read(&mut caller, offset, len, out_ptr)
                },
            )
            .map_err(|e| Error::Wasm(e.to_string()))?;

        // Payload write function
        linker
            .func_wrap(
                "host",
                "payload_write",
                |mut caller: Caller<'_, HostState>, offset: i32, len: i32, data_ptr: i32| -> i32 {
                    Self::payload_write(&mut caller, offset, len, data_ptr)
                },
            )
            .map_err(|e| Error::Wasm(e.to_string()))?;

        Ok(())
    }

    /// HTTP request host function.
    fn http_request(
        caller: &mut Caller<'_, HostState>,
        url_ptr: i32,
        url_len: i32,
        method_ptr: i32,
        method_len: i32,
    ) -> i32 {
        let url = match Self::read_string_from_memory(caller, url_ptr, url_len) {
            Ok(s) => s,
            Err(_) => return -1,
        };

        let _method = match Self::read_string_from_memory(caller, method_ptr, method_len) {
            Ok(s) => s,
            Err(_) => return -1,
        };

        let allowed = {
            let state = caller.data();
            state.is_host_allowed(&url)
        };

        if !allowed {
            tracing::warn!("Host not allowed: {}", url);
            return -2;
        }

        let client = match reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
        {
            Ok(c) => c,
            Err(_) => return -3,
        };

        match client.get(&url).send() {
            Ok(response) => {
                let status = response.status().as_u16() as i32;

                match response.text() {
                    Ok(body) => {
                        let state = caller.data_mut();
                        if let Ok(mut guard) = state.last_response.lock() {
                            *guard = body.into_bytes();
                        }
                        status
                    }
                    Err(_) => -4,
                }
            }
            Err(e) => {
                tracing::error!("HTTP request failed: {}", e);
                -5
            }
        }
    }

    /// Get last response size.
    fn last_response_size(caller: &Caller<'_, HostState>) -> i32 {
        match caller.data().last_response.lock() {
            Ok(guard) => guard.len() as i32,
            Err(_) => -1,
        }
    }

    /// Read last response into guest memory.
    fn last_response_read(caller: &mut Caller<'_, HostState>, out_ptr: i32, max_len: i32) -> i32 {
        let data = match caller.data().last_response.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => return -1,
        };

        let write_len = data.len().min(max_len as usize);
        match Self::write_bytes_to_memory(caller, out_ptr, &data[..write_len]) {
            Ok(_) => write_len as i32,
            Err(_) => -1,
        }
    }

    /// Get payload size.
    fn payload_size(caller: &Caller<'_, HostState>) -> i32 {
        match caller.data().payload_buffer.lock() {
            Ok(guard) => guard.len() as i32,
            Err(_) => -1,
        }
    }

    /// Read payload bytes into WASM memory.
    fn payload_read(
        caller: &mut Caller<'_, HostState>,
        offset: i32,
        len: i32,
        out_ptr: i32,
    ) -> i32 {
        let payload = match caller.data().payload_buffer.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => return -1,
        };

        let offset = offset as usize;
        let len = len as usize;

        if offset >= payload.len() {
            return 0;
        }

        let end = (offset + len).min(payload.len());
        let data = &payload[offset..end];

        match Self::write_bytes_to_memory(caller, out_ptr, data) {
            Ok(_) => data.len() as i32,
            Err(_) => -1,
        }
    }

    /// Write payload bytes from WASM memory.
    fn payload_write(
        caller: &mut Caller<'_, HostState>,
        offset: i32,
        len: i32,
        data_ptr: i32,
    ) -> i32 {
        let data = match Self::read_bytes_from_memory(caller, data_ptr, len) {
            Ok(d) => d,
            Err(_) => return -1,
        };

        let state = caller.data_mut();

        match state.updated_payload.lock() {
            Ok(mut guard) => {
                if let Some(ref mut payload) = *guard {
                    let offset = offset as usize;
                    if offset + data.len() <= payload.len() {
                        payload[offset..offset + data.len()].copy_from_slice(&data);
                    } else {
                        payload.resize(offset + data.len(), 0);
                        payload[offset..].copy_from_slice(&data);
                    }
                } else {
                    let mut new_payload = vec![0u8; offset as usize + data.len()];
                    new_payload[offset as usize..].copy_from_slice(&data);
                    *guard = Some(new_payload);
                }
                data.len() as i32
            }
            Err(_) => -1,
        }
    }

    /// Helper: Read string from WASM memory.
    fn read_string_from_memory(
        caller: &mut Caller<'_, HostState>,
        ptr: i32,
        len: i32,
    ) -> Result<String> {
        let bytes = Self::read_bytes_from_memory(caller, ptr, len)?;
        String::from_utf8(bytes).map_err(|_| Error::InvalidInput("Invalid UTF-8".to_string()))
    }

    /// Helper: Read bytes from WASM memory.
    fn read_bytes_from_memory(
        caller: &mut Caller<'_, HostState>,
        ptr: i32,
        len: i32,
    ) -> Result<Vec<u8>> {
        let memory = Self::get_memory(caller)?;
        let mut buffer = vec![0u8; len as usize];
        memory
            .read(caller, ptr as usize, &mut buffer)
            .map_err(|e| Error::Wasm(e.to_string()))?;
        Ok(buffer)
    }

    /// Helper: Write bytes to WASM memory.
    fn write_bytes_to_memory(
        caller: &mut Caller<'_, HostState>,
        ptr: i32,
        data: &[u8],
    ) -> Result<()> {
        let memory = Self::get_memory(caller)?;
        memory
            .write(caller, ptr as usize, data)
            .map_err(|e| Error::Wasm(e.to_string()))?;
        Ok(())
    }

    /// Helper: Get memory export from WASM instance.
    fn get_memory(caller: &mut Caller<'_, HostState>) -> Result<Memory> {
        caller
            .get_export("memory")
            .and_then(|ext| ext.into_memory())
            .ok_or_else(|| Error::Wasm("Memory export not found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_allowed() {
        let state = HostState::new(
            "/test.sync".to_string(),
            vec![
                "api.example.com".to_string(),
                "*.internal.local".to_string(),
            ],
            30,
        );

        assert!(state.is_host_allowed("https://api.example.com/path"));
        assert!(state.is_host_allowed("http://sub.internal.local:8080/"));
        assert!(!state.is_host_allowed("https://evil.com/"));
        assert!(!state.is_host_allowed("https://other.example.com/"));
    }
}
