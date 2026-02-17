//! # sync-format
//!
//! Core library for `.sync` format parsing and creation.
//!
//! This crate provides:
//! - `.sync` archive reading and writing
//! - `manifest.toml` parsing and validation
//! - Archive builder for creating new `.sync` files
//! - Encryption/decryption support (with `encryption` feature)
//! - Signature verification (with `signatures` feature)
//!
//! ## Features
//!
//! - `signatures` (default): Ed25519 signature verification
//! - `encryption`: age-based payload encryption/decryption for vault archives
//! - `crypto`: Enables both `signatures` and `encryption`
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
#[cfg(feature = "signatures")]
pub mod verification;

pub use builder::SyncBuilder;
pub use error::{Error, Result};
#[cfg(feature = "encryption")]
pub use format::{decrypt_data, encrypt_data};
pub use format::{SyncArchive, SyncEntry};
pub use manifest::{
    EncryptionMeta, Manifest, ManifestCapabilities, ManifestEncryption, ManifestMetadata,
    ManifestOwnership, ManifestPermissions, ManifestPolicy, ManifestSignature,
    ManifestVerification, NetworkScope, SyncManifest, SyncSection, SyncVariant,
};
#[cfg(feature = "signatures")]
pub use verification::{
    compute_content_hash, verify_manifest_signature, verify_sync_file, ManifestSignatureResult,
    SyncSignature, VerificationResult,
};

// Re-export secrecy for consumers using the encryption feature
#[cfg(feature = "encryption")]
pub use secrecy::SecretString;

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
