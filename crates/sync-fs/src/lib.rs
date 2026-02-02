//! # sync-fs
//!
//! Virtual filesystem abstraction for `.sync` archives.
//!
//! This crate provides:
//! - VFS mounting configuration
//! - Virtual file entries from `.sync` payloads
//! - Display name and path generation
//!
//! ## Example
//!
//! ```ignore
//! use sync_format::SyncArchive;
//! use sync_fs::{VfsMount, VfsMountConfig};
//! use std::path::PathBuf;
//!
//! let archive = SyncArchive::open("example.sync")?;
//! let config = VfsMountConfig {
//!     mount_path: PathBuf::from("/mnt"),
//!     expose_as_read_only: true,
//!     show_original_extension: true,
//! };
//! let mount = VfsMount::from_archive(&archive, config)?;
//!
//! for entry in mount.entries() {
//!     println!("{}: {}", entry.display_name, entry.vfs_path.display());
//! }
//! ```

mod vfs;

pub use vfs::{VfsEntry, VfsMount, VfsMountConfig};

// Re-export sync-format types for convenience
pub use sync_format::{SyncArchive, SyncEntry};
