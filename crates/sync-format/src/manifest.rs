use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// The `[sync]` section of the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSection {
    /// Format version (e.g., "1.2").
    pub version: String,
    /// MIME type of the payload content.
    pub content_type: String,
    /// Display extension for the payload (e.g., "txt", "csv").
    pub display_ext: String,
}

/// The `[meta]` section of the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestMetadata {
    /// Identifier of the creator.
    pub created_by: String,
    /// ISO 8601 timestamp of creation.
    pub created_at: String,
    /// Hash algorithm used (e.g., "blake3").
    pub hash_algo: String,
}

/// The `[policy]` section of the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestPolicy {
    /// Time-to-live in seconds.
    pub ttl: u64,
    /// Execution timeout in seconds.
    pub timeout: u64,
}

/// The `[permissions]` section of the manifest.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManifestPermissions {
    /// List of allowed network hosts.
    #[serde(default)]
    pub allow_hosts: Vec<String>,
    /// List of allowed environment variables.
    #[serde(default)]
    pub allow_env: Vec<String>,
}

/// The `[ownership]` section of the manifest.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManifestOwnership {
    /// Optional owner capsule identifier.
    #[serde(default)]
    pub owner_capsule: Option<String>,
    /// Whether writing is allowed.
    #[serde(default)]
    pub write_allowed: bool,
}

/// The `[verification]` section of the manifest.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManifestVerification {
    /// Whether verification is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Type of VM for verification.
    #[serde(default)]
    pub vm_type: Option<String>,
    /// Type of proof for verification.
    #[serde(default)]
    pub proof_type: Option<String>,
}

/// Complete manifest structure for a `.sync` archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncManifest {
    /// The sync format section.
    pub sync: SyncSection,
    /// Metadata section.
    pub meta: ManifestMetadata,
    /// Policy section.
    pub policy: ManifestPolicy,
    /// Permissions section.
    #[serde(default)]
    pub permissions: ManifestPermissions,
    /// Ownership section.
    #[serde(default)]
    pub ownership: ManifestOwnership,
    /// Verification section.
    #[serde(default)]
    pub verification: ManifestVerification,
}

/// Network scope for share policy decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkScope {
    /// Local network (trusted).
    Local,
    /// Wide area network (untrusted).
    Wan,
}

/// Type alias for backward compatibility.
pub type Manifest = SyncManifest;

impl SyncManifest {
    /// Parse a manifest from TOML bytes.
    pub fn from_toml(data: &[u8]) -> crate::Result<Self> {
        let text = std::str::from_utf8(data)
            .map_err(|e| crate::Error::ManifestError(format!("Invalid UTF-8: {}", e)))?;
        toml::from_str(text).map_err(|e| crate::Error::TomlError(e.to_string()))
    }

    /// Get the creation timestamp.
    pub fn get_created_at(&self) -> crate::Result<DateTime<Utc>> {
        self.meta
            .created_at
            .parse()
            .map_err(|e| crate::Error::ManifestError(format!("Invalid created_at: {}", e)))
    }

    /// Check if the archive has expired based on TTL.
    pub fn is_expired(&self) -> crate::Result<bool> {
        let created_at = self.get_created_at()?;
        let expires_at = created_at + Duration::seconds(self.policy.ttl as i64);
        Ok(Utc::now() > expires_at)
    }

    /// Get the remaining time until expiration.
    pub fn expires_in(&self) -> crate::Result<Duration> {
        let created_at = self.get_created_at()?;
        let expires_at = created_at + Duration::seconds(self.policy.ttl as i64);
        let now = Utc::now();

        if now > expires_at {
            Ok(Duration::zero())
        } else {
            Ok(expires_at - now)
        }
    }
}
