//! FUSE filesystem adapter for `.sync` archives.
//!
//! This module provides a FUSE (Filesystem in Userspace) adapter that allows
//! `.sync` archives to be mounted as read-only filesystems, enabling direct
//! access from Finder, VS Code, or any other application.
//!
//! # Features
//!
//! - **Zero-Copy Reads**: Uses `pread()` to read directly from the archive
//!   without buffering the entire file
//! - **Thread-Safe**: Multiple concurrent reads are supported via `pread()`
//! - **Minimal Memory**: Only metadata is kept in memory, not file contents
//!
//! # Example
//!
//! ```ignore
//! use sync_fs::fuse::{SyncFuseFS, mount};
//! use sync_fs::{VfsMount, VfsMountConfig};
//! use sync_format::SyncArchive;
//!
//! let archive = SyncArchive::open("example.sync")?;
//! let config = VfsMountConfig::default();
//! let vfs = VfsMount::from_archive(&archive, config)?;
//!
//! mount("example.sync", "/mnt/sync", vfs)?;
//! ```

mod adapter;

pub use adapter::*;
