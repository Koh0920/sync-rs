//! WebDAV server implementation using hyper.
//!
//! This module provides the HTTP server that hosts the WebDAV filesystem,
//! allowing clients to connect and mount the archive.

use super::SyncDavFs;
use crate::vfs::VfsMount;
use dav_server::{fakels::FakeLs, DavHandler};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use log::{debug, error, info};
use std::convert::Infallible;
use std::io;
use std::net::SocketAddr;
use std::path::Path;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// WebDAV server for `.sync` archives.
pub struct SyncWebDavServer {
    /// Server address.
    addr: SocketAddr,
    /// Shutdown signal sender.
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl SyncWebDavServer {
    /// Get the server's listen address.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Get the URL to mount this server.
    pub fn mount_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Shutdown the server.
    pub fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// Start a WebDAV server and block until shutdown.
///
/// # Arguments
///
/// * `archive_path` - Path to the `.sync` archive file
/// * `vfs` - Pre-built VFS mount with file metadata
/// * `port` - Port to listen on (0 for auto-assign)
///
/// # Example
///
/// ```ignore
/// use sync_fs::webdav::serve;
/// use sync_fs::{VfsMount, VfsMountConfig};
/// use sync_format::SyncArchive;
///
/// #[tokio::main]
/// async fn main() -> std::io::Result<()> {
///     let archive = SyncArchive::open("data.sync").unwrap();
///     let vfs = VfsMount::from_archive(&archive, VfsMountConfig::default()).unwrap();
///     
///     // This blocks until Ctrl+C
///     serve("data.sync", vfs, 4918).await
/// }
/// ```
pub async fn serve<P: AsRef<Path>>(archive_path: P, vfs: VfsMount, port: u16) -> io::Result<()> {
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let fs = SyncDavFs::new(vfs, archive_path.as_ref().to_path_buf());

    // Build WebDAV handler
    let dav_server = DavHandler::builder()
        .filesystem(Box::new(fs))
        .locksystem(FakeLs::new()) // Fake locks for macOS/Windows compatibility
        .build_handler();

    let listener = TcpListener::bind(addr).await?;
    let local_addr = listener.local_addr()?;

    info!("WebDAV server listening on http://{}", local_addr);
    info!("");
    info!("To mount in Finder:");
    info!("  1. Open Finder");
    info!("  2. Press Cmd+K (Go → Connect to Server)");
    info!("  3. Enter: http://{}", local_addr);
    info!("  4. Click Connect");
    info!("");
    info!("To mount from terminal:");
    info!("  mkdir -p /tmp/sync-mount");
    info!("  mount_webdav http://{} /tmp/sync-mount", local_addr);
    info!("");
    info!("Press Ctrl+C to stop the server");

    loop {
        let (stream, remote_addr) = listener.accept().await?;
        debug!("Connection from {}", remote_addr);

        let dav_server = dav_server.clone();
        let io = TokioIo::new(stream);

        tokio::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(
                    io,
                    service_fn(move |req| {
                        let dav_server = dav_server.clone();
                        async move { Ok::<_, Infallible>(dav_server.handle(req).await) }
                    }),
                )
                .await
            {
                error!("Connection error: {:?}", err);
            }
        });
    }
}

/// Start a WebDAV server in the background.
///
/// Returns a handle that can be used to get the server address and shut it down.
///
/// # Arguments
///
/// * `archive_path` - Path to the `.sync` archive file
/// * `vfs` - Pre-built VFS mount with file metadata
/// * `port` - Port to listen on (0 for auto-assign)
///
/// # Example
///
/// ```ignore
/// use sync_fs::webdav::serve_background;
/// use sync_fs::{VfsMount, VfsMountConfig};
/// use sync_format::SyncArchive;
///
/// #[tokio::main]
/// async fn main() -> std::io::Result<()> {
///     let archive = SyncArchive::open("data.sync").unwrap();
///     let vfs = VfsMount::from_archive(&archive, VfsMountConfig::default()).unwrap();
///     
///     let server = serve_background("data.sync", vfs, 0).await?;
///     println!("Server running at {}", server.mount_url());
///     
///     // Do other work...
///     
///     // Shutdown when done
///     server.shutdown();
///     Ok(())
/// }
/// ```
pub async fn serve_background<P: AsRef<Path>>(
    archive_path: P,
    vfs: VfsMount,
    port: u16,
) -> io::Result<SyncWebDavServer> {
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let fs = SyncDavFs::new(vfs, archive_path.as_ref().to_path_buf());

    // Build WebDAV handler
    let dav_server = DavHandler::builder()
        .filesystem(Box::new(fs))
        .locksystem(FakeLs::new())
        .build_handler();

    let listener = TcpListener::bind(addr).await?;
    let local_addr = listener.local_addr()?;

    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

    info!("WebDAV server started on http://{}", local_addr);

    tokio::spawn(async move {
        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, remote_addr)) => {
                            debug!("Connection from {}", remote_addr);
                            let dav_server = dav_server.clone();
                            let io = TokioIo::new(stream);

                            tokio::spawn(async move {
                                if let Err(err) = http1::Builder::new()
                                    .serve_connection(
                                        io,
                                        service_fn(move |req| {
                                            let dav_server = dav_server.clone();
                                            async move {
                                                Ok::<_, Infallible>(dav_server.handle(req).await)
                                            }
                                        }),
                                    )
                                    .await
                                {
                                    error!("Connection error: {:?}", err);
                                }
                            });
                        }
                        Err(e) => {
                            error!("Accept error: {:?}", e);
                        }
                    }
                }
                _ = &mut shutdown_rx => {
                    info!("WebDAV server shutting down");
                    break;
                }
            }
        }
    });

    Ok(SyncWebDavServer {
        addr: local_addr,
        shutdown_tx: Some(shutdown_tx),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_starts() {
        // This test would require a real .sync file
        // For now, just verify the module compiles
    }
}
