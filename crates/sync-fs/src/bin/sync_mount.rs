//! sync-mount: Mount `.sync` archives via WebDAV server.
//!
//! This binary starts a local WebDAV server that exposes `.sync` archives,
//! enabling direct access from Finder, Windows Explorer, or any WebDAV client.
//!
//! # Usage
//!
//! ```bash
//! # Start WebDAV server
//! sync-mount example.sync
//!
//! # Then mount in Finder: Cmd+K → http://localhost:4918
//! ```

use clap::Parser;
use env_logger::Env;
use log::{error, info};
use std::path::PathBuf;
use std::process;
use sync_format::SyncArchive;
use sync_fs::webdav;
use sync_fs::{VfsMount, VfsMountConfig};

/// Mount .sync archives via WebDAV server.
///
/// Start a local WebDAV server that can be mounted from Finder (Cmd+K),
/// Windows Explorer, or any WebDAV-compatible client.
#[derive(Parser, Debug)]
#[command(name = "sync-mount")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the .sync archive file
    #[arg(value_name = "ARCHIVE")]
    archive: PathBuf,

    /// Port to listen on (default: 4918)
    #[arg(short, long, default_value = "4918")]
    port: u16,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(Env::default().default_filter_or(log_level))
        .format_timestamp_millis()
        .init();

    // Validate archive path
    if !args.archive.exists() {
        error!("Archive not found: {}", args.archive.display());
        process::exit(1);
    }

    if !args.archive.is_file() {
        error!("Not a file: {}", args.archive.display());
        process::exit(1);
    }

    // Open the archive
    info!("Opening archive: {}", args.archive.display());
    let archive = match SyncArchive::open(&args.archive) {
        Ok(a) => a,
        Err(e) => {
            error!("Failed to open archive: {}", e);
            process::exit(1);
        }
    };

    // Display archive info
    let manifest = archive.manifest();
    info!("Content-Type: {}", manifest.sync.content_type);
    info!("Display Extension: {}", manifest.sync.display_ext);

    if let Some(entry) = archive.payload_entry() {
        info!("Payload size: {} bytes", entry.size);
    }

    // Create VFS mount
    let config = VfsMountConfig::default();
    let vfs = match VfsMount::from_archive(&archive, config) {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to create VFS mount: {}", e);
            process::exit(1);
        }
    };

    info!("Entries: {}", vfs.entries().len());
    for entry in vfs.entries() {
        info!("  - {} ({} bytes)", entry.display_name, entry.size);
    }

    // Start WebDAV server
    if let Err(e) = webdav::serve(&args.archive, vfs, args.port).await {
        error!("Server error: {}", e);
        process::exit(1);
    }
}
