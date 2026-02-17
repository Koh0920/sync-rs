//! Remote WebDAV mount via Tailnet.
//!
//! This module provides P2P-based WebDAV mounting using the Tailscale network.

use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Remote mount configuration
#[derive(Debug, Clone)]
pub struct RemoteMountConfig {
    /// Remote Tailnet address (e.g., "device.tailnet:4918")
    pub remote_addr: String,
    /// Local SOCKS5 proxy port for Tailnet
    pub socks_port: u16,
    /// Connection timeout
    pub timeout: Duration,
    /// Enable local caching
    pub cache_enabled: bool,
    /// Local cache directory
    pub cache_dir: Option<PathBuf>,
}

impl Default for RemoteMountConfig {
    fn default() -> Self {
        Self {
            remote_addr: String::new(),
            socks_port: 0,
            timeout: Duration::from_secs(30),
            cache_enabled: true,
            cache_dir: None,
        }
    }
}

/// Remote mount handle
pub struct RemoteMount {
    config: RemoteMountConfig,
    local_port: u16,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl RemoteMount {
    /// Create a new remote mount
    pub async fn connect(config: RemoteMountConfig) -> io::Result<Self> {
        // Validate config
        if config.remote_addr.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "remote_addr is required",
            ));
        }
        if config.socks_port == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "socks_port is required (check Tailnet status)",
            ));
        }

        // Test connection through SOCKS proxy
        test_socks_connection(&config).await?;

        // Start local proxy server
        let (local_port, shutdown_tx) = start_local_proxy(&config).await?;

        Ok(Self {
            config,
            local_port,
            shutdown_tx: Some(shutdown_tx),
        })
    }

    /// Get local mount URL
    pub fn mount_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.local_port)
    }

    /// Get remote address
    pub fn remote_addr(&self) -> &str {
        &self.config.remote_addr
    }

    /// Disconnect and cleanup
    pub fn disconnect(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for RemoteMount {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// Test connection to remote through SOCKS proxy
async fn test_socks_connection(config: &RemoteMountConfig) -> io::Result<()> {
    use tokio::time::timeout;

    let socks_addr: SocketAddr = ([127, 0, 0, 1], config.socks_port).into();

    let result = timeout(config.timeout, async {
        let mut stream = TcpStream::connect(socks_addr).await?;

        // SOCKS5 handshake
        // Version (5), Methods count (1), No auth (0)
        stream.write_all(&[0x05, 0x01, 0x00]).await?;

        let mut response = [0u8; 2];
        stream.read_exact(&mut response).await?;

        if response[0] != 0x05 || response[1] != 0x00 {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                "SOCKS5 handshake failed",
            ));
        }

        // Parse remote address
        let (host, port) = parse_addr(&config.remote_addr)?;

        // SOCKS5 connect request
        let mut request = Vec::new();
        request.push(0x05); // Version
        request.push(0x01); // CMD: Connect
        request.push(0x00); // Reserved
        request.push(0x03); // ATYP: Domain name
        request.push(host.len() as u8);
        request.extend_from_slice(host.as_bytes());
        request.push((port >> 8) as u8);
        request.push((port & 0xff) as u8);

        stream.write_all(&request).await?;

        // Read response (at least 10 bytes)
        let mut connect_response = [0u8; 10];
        stream.read_exact(&mut connect_response).await?;

        if connect_response[1] != 0x00 {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                format!("SOCKS5 connect failed: error {}", connect_response[1]),
            ));
        }

        Ok(())
    })
    .await;

    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "Connection timeout",
        )),
    }
}

fn parse_addr(addr: &str) -> io::Result<(String, u16)> {
    let parts: Vec<&str> = addr.rsplitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid address format, expected host:port",
        ));
    }
    let port: u16 = parts[0]
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid port"))?;
    let host = parts[1].to_string();
    Ok((host, port))
}

/// Start local TCP proxy that tunnels through SOCKS5
async fn start_local_proxy(
    config: &RemoteMountConfig,
) -> io::Result<(u16, tokio::sync::oneshot::Sender<()>)> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let local_addr = listener.local_addr()?;
    let local_port = local_addr.port();

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

    let socks_port = config.socks_port;
    let remote_addr = config.remote_addr.clone();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((client, _)) => {
                            let remote = remote_addr.clone();
                            tokio::spawn(async move {
                                if let Err(e) = proxy_connection(client, socks_port, &remote).await {
                                    log::error!("Proxy connection error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            log::error!("Accept error: {}", e);
                        }
                    }
                }
                _ = &mut shutdown_rx => {
                    break;
                }
            }
        }
    });

    Ok((local_port, shutdown_tx))
}

/// Proxy a single connection through SOCKS5
async fn proxy_connection(
    mut client: TcpStream,
    socks_port: u16,
    remote_addr: &str,
) -> io::Result<()> {
    let socks_addr: SocketAddr = ([127, 0, 0, 1], socks_port).into();
    let mut socks = TcpStream::connect(socks_addr).await?;

    // SOCKS5 handshake
    socks.write_all(&[0x05, 0x01, 0x00]).await?;
    let mut response = [0u8; 2];
    socks.read_exact(&mut response).await?;

    if response[0] != 0x05 || response[1] != 0x00 {
        return Err(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            "SOCKS5 handshake failed",
        ));
    }

    // Connect request
    let (host, port) = parse_addr(remote_addr)?;
    let mut request = Vec::new();
    request.push(0x05);
    request.push(0x01);
    request.push(0x00);
    request.push(0x03);
    request.push(host.len() as u8);
    request.extend_from_slice(host.as_bytes());
    request.push((port >> 8) as u8);
    request.push((port & 0xff) as u8);

    socks.write_all(&request).await?;

    let mut connect_response = [0u8; 10];
    socks.read_exact(&mut connect_response).await?;

    if connect_response[1] != 0x00 {
        return Err(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("SOCKS5 connect failed: {}", connect_response[1]),
        ));
    }

    // Bidirectional copy
    let (mut client_read, mut client_write) = client.split();
    let (mut socks_read, mut socks_write) = socks.split();

    let client_to_socks = tokio::io::copy(&mut client_read, &mut socks_write);
    let socks_to_client = tokio::io::copy(&mut socks_read, &mut client_write);

    tokio::select! {
        _ = client_to_socks => {}
        _ = socks_to_client => {}
    }

    Ok(())
}

/// Cache configuration for remote mounts
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum cache size in bytes
    pub max_size_bytes: u64,
    /// Time-to-live for cached entries
    pub ttl: Duration,
    /// Cache directory
    pub cache_dir: PathBuf,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size_bytes: 100 * 1024 * 1024, // 100MB
            ttl: Duration::from_secs(3600),    // 1 hour
            cache_dir: std::env::temp_dir().join("capsule-cache"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_addr() {
        let (host, port) = parse_addr("example.com:4918").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 4918);

        let (host, port) = parse_addr("device.tailnet:8080").unwrap();
        assert_eq!(host, "device.tailnet");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_addr_invalid() {
        assert!(parse_addr("no-port").is_err());
        assert!(parse_addr("host:invalid").is_err());
    }

    #[test]
    fn test_remote_mount_config_default() {
        let config = RemoteMountConfig::default();
        assert!(config.remote_addr.is_empty());
        assert_eq!(config.socks_port, 0);
        assert!(config.cache_enabled);
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert_eq!(config.max_size_bytes, 100 * 1024 * 1024);
        assert_eq!(config.ttl, Duration::from_secs(3600));
        assert!(config.cache_dir.to_string_lossy().contains("capsule-cache"));
    }

    #[test]
    fn test_parse_addr_with_ipv4() {
        let (host, port) = parse_addr("192.168.1.1:8080").unwrap();
        assert_eq!(host, "192.168.1.1");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_addr_with_subdomain() {
        let (host, port) = parse_addr("my.device.tailnet.ts.net:4918").unwrap();
        assert_eq!(host, "my.device.tailnet.ts.net");
        assert_eq!(port, 4918);
    }

    #[test]
    fn test_parse_addr_port_zero() {
        let (host, port) = parse_addr("example.com:0").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 0);
    }

    #[test]
    fn test_parse_addr_port_max() {
        let (host, port) = parse_addr("example.com:65535").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 65535);
    }

    #[test]
    fn test_parse_addr_port_overflow() {
        // 65536 is too large for u16
        assert!(parse_addr("example.com:65536").is_err());
    }
}
