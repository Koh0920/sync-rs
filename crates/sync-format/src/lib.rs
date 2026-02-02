//! # sync-format
//!
//! Core library for `.sync` format parsing and creation.
//!
//! This crate provides:
//! - `.sync` archive reading and writing
//! - `manifest.toml` parsing and validation
//! - Archive builder for creating new `.sync` files
//!
//! ## Example
//!
//! ```ignore
//! use sync_format::{SyncArchive, SyncBuilder, SyncManifest};
//!
//! // Open an existing archive
//! let archive = SyncArchive::open("example.sync")?;
//! let manifest = archive.manifest();
//!
//! // Create a new archive
//! SyncBuilder::new()
//!     .with_manifest(manifest.clone())
//!     .with_payload_bytes(b"hello")
//!     .with_wasm_bytes(b"\0asm\x01\0\0\0")
//!     .write_to("new.sync")?;
//! ```

mod builder;
mod error;
mod format;
mod manifest;

pub use builder::SyncBuilder;
pub use error::{Error, Result};
pub use format::{SyncArchive, SyncEntry};
pub use manifest::{
    Manifest, ManifestMetadata, ManifestOwnership, ManifestPermissions, ManifestPolicy,
    ManifestVerification, NetworkScope, SyncManifest, SyncSection,
};

/// Policy for sharing `.sync` archives across networks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharePolicy {
    /// Share only the logic (WASM) without data snapshot.
    LogicOnly,
    /// Share with verified data snapshot.
    VerifiedSnapshot,
}

impl SharePolicy {
    /// Determine the appropriate share policy for a network scope.
    pub fn for_network(scope: NetworkScope) -> Self {
        match scope {
            NetworkScope::Local => SharePolicy::LogicOnly,
            NetworkScope::Wan => SharePolicy::VerifiedSnapshot,
        }
    }
}
