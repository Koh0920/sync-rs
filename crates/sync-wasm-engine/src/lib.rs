//! # sync-wasm-engine
//!
//! WASM runtime engine for executing `sync.wasm` modules from `.sync` archives.
//!
//! This crate uses [wasmtime](https://wasmtime.dev/) to provide in-process WASM
//! execution with host functions for HTTP requests, payload I/O, and more.
//!
//! ## Difference from `sync-runtime`
//!
//! - **`sync-runtime`** (in `apps/sync-rs/crates/sync-runtime`): Process-based
//!   guest session management. Communicates with host apps via JSON over stdin/stdout.
//! - **`sync-wasm-engine`** (this crate): In-process WASM execution using wasmtime.
//!   Directly executes `sync.wasm` modules with host function bindings.
//!
//! ## Example
//!
//! ```ignore
//! use sync_wasm_engine::{SyncRuntime, ExecutionConfig, WasmRunner};
//! use sync_format::SyncArchive;
//!
//! let mut archive = SyncArchive::open("example.sync")?;
//! let mut runtime = SyncRuntime::new()?;
//! let result = runtime.execute(&mut archive, "ReadPayload", None)?;
//!
//! if result.success {
//!     println!("Execution succeeded: {:?}", result.result);
//! }
//! ```

pub mod error;
pub mod host;
pub mod runner;

pub use error::{Error, Result};
pub use host::{HostFunctions, HostState};
pub use runner::{ExecutionConfig, WasmRunner};

use sync_format::SyncArchive;

/// Main entry point for executing `sync.wasm` modules.
pub struct SyncRuntime {
    runner: WasmRunner,
}

impl SyncRuntime {
    /// Create a new `SyncRuntime` with default configuration.
    pub fn new() -> Result<Self> {
        Ok(Self {
            runner: WasmRunner::new()?,
        })
    }

    /// Execute a WASM action on the given archive.
    pub fn execute(
        &mut self,
        archive: &mut SyncArchive,
        action: &str,
        input: Option<serde_json::Value>,
    ) -> Result<ExecutionResult> {
        self.runner.execute(archive, action, input)
    }
}

/// Result of a WASM execution.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Whether the execution succeeded.
    pub success: bool,
    /// Optional result data.
    pub result: Option<serde_json::Value>,
    /// Optional error message.
    pub error: Option<String>,
    /// Whether the payload was updated during execution.
    pub payload_updated: bool,
}
