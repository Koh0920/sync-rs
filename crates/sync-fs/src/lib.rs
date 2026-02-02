//! # sync-fs
//!
//! Virtual filesystem abstraction for `.sync` archives.
//!
//! This crate provides:
//! - VFS mounting configuration
//! - Virtual file entries from `.sync` payloads
//! - Display name and path generation
//! - **WebDAV server support** (recommended, with `webdav` feature)
//! - FUSE filesystem support (optional, with `fuse` feature)
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
//!
//! ## WebDAV Support (Recommended)
//!
//! Enable the `webdav` feature to start a WebDAV server that can be mounted
//! from Finder (macOS), Explorer (Windows), or any WebDAV client:
//!
//! ```ignore
//! use sync_fs::webdav::serve;
//! use sync_fs::{VfsMount, VfsMountConfig};
//! use sync_format::SyncArchive;
//!
//! #[tokio::main]
//! async fn main() -> std::io::Result<()> {
//!     let archive = SyncArchive::open("data.sync").unwrap();
//!     let vfs = VfsMount::from_archive(&archive, VfsMountConfig::default()).unwrap();
//!     
//!     // Start server on port 4918 (blocks until Ctrl+C)
//!     serve("data.sync", vfs, 4918).await
//! }
//! ```
//!
//! **Advantages over FUSE:**
//! - No kernel extensions needed (works on Apple Silicon without changes)
//! - Cross-platform (macOS, Windows, Linux)
//! - Standard HTTP/WebDAV protocol
//!
//! ## FUSE Support (Optional)
//!
//! For power users on Linux, enable the `fuse` feature:
//!
//! ```ignore
//! use sync_fs::fuse::mount;
//! use sync_fs::{VfsMount, VfsMountConfig};
//! use sync_format::SyncArchive;
//!
//! let archive = SyncArchive::open("data.sync")?;
//! let vfs = VfsMount::from_archive(&archive, VfsMountConfig::default())?;
//! mount("data.sync", "/mnt/data", vfs)?;
//! ```

mod vfs;

#[cfg(feature = "webdav")]
pub mod webdav;

#[cfg(feature = "fuse")]
pub mod fuse;

pub use vfs::{VfsEntry, VfsMount, VfsMountConfig};

// Re-export sync-format types for convenience
pub use sync_format::{SyncArchive, SyncEntry};
