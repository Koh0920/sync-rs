//! WebDAV server adapter for `.sync` archives.
//!
//! This module provides a WebDAV server that exposes `.sync` archives as
//! network-mountable filesystems, enabling direct access from Finder, Windows
//! Explorer, or any WebDAV-compatible client.
//!
//! # Advantages over FUSE
//!
//! - **No kernel extensions**: Works on macOS without macFUSE
//! - **Apple Silicon support**: No security policy changes needed
//! - **Cross-platform**: Works on macOS, Windows, and Linux out of the box
//! - **Standard protocol**: Uses HTTP/WebDAV (RFC 4918)
//!
//! # Example
//!
//! ```ignore
//! use sync_fs::webdav::{SyncDavFs, serve};
//! use sync_fs::{VfsMount, VfsMountConfig};
//! use sync_format::SyncArchive;
//!
//! let archive = SyncArchive::open("example.sync")?;
//! let config = VfsMountConfig::default();
//! let vfs = VfsMount::from_archive(&archive, config)?;
//!
//! // Start WebDAV server on port 4918
//! serve("example.sync", vfs, 4918).await?;
//! ```

mod filesystem;
mod remote;
mod server;
mod writable;

pub use filesystem::SyncDavFs;
pub use remote::{CacheConfig, RemoteMount, RemoteMountConfig};
pub use server::{
    serve, serve_background, serve_dual_background, serve_writable, serve_writable_background,
    SyncWebDavServer,
};
pub use writable::WritableSyncFs;
