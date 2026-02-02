//! # sync-runtime
//!
//! WASM execution runtime and guest session management for `.sync` archives.
//!
//! This crate provides:
//! - Guest session lifecycle management
//! - WASM module execution with sandboxing
//! - Permission enforcement and policy application
//!
//! ## Example
//!
//! ```ignore
//! use sync_runtime::{GuestSession, GuestAction, WidgetBounds};
//!
//! let mut session = GuestSession::new("example.sync".into())?;
//! session.as_owner()?;
//! session.grant_wasm_execution()?;
//! session.set_widget_bounds(WidgetBounds { x: 0, y: 0, width: 800, height: 600 })?;
//!
//! let response = session.execute_wasm()?;
//! ```

mod guest;
mod session;

pub use guest::{
    decode_payload_base64, encode_payload_base64, GuestAction, GuestContext, GuestContextRole,
    GuestError, GuestErrorCode, GuestMode, GuestPermission, GuestRequest, GuestResponse,
    GUEST_PROTOCOL_VERSION,
};
pub use session::{GuestSession, WidgetBounds};

// Re-export sync-format types for convenience
pub use sync_format::{ManifestPermissions, SyncArchive, SyncBuilder, SyncEntry, SyncManifest};
