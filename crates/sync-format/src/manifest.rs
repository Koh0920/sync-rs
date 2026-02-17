use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Sync variant type (v1.3).
///
/// Determines how the `.sync` archive is treated:
/// - `Plain`: Standard unencrypted data (default)
/// - `Vault`: Encrypted vault mode (requires `encryption` feature)
/// - `App`: Application capsule
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SyncVariant {
    /// Plain unencrypted data (default).
    #[serde(rename = "plain")]
    #[default]
    Plain,
    /// Encrypted vault mode.
    #[serde(rename = "vault")]
    Vault,
    /// Application capsule.
    #[serde(rename = "app")]
    App,
    /// Generic data (written by Capsule apps via Host Bridge).
    #[serde(rename = "data")]
    Data,
}

impl std::fmt::Display for SyncVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncVariant::Plain => write!(f, "plain"),
            SyncVariant::Vault => write!(f, "vault"),
            SyncVariant::App => write!(f, "app"),
            SyncVariant::Data => write!(f, "data"),
        }
    }
}

impl std::str::FromStr for SyncVariant {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "plain" => Ok(SyncVariant::Plain),
            "vault" => Ok(SyncVariant::Vault),
            "app" => Ok(SyncVariant::App),
            "data" => Ok(SyncVariant::Data),
            _ => Err(format!("Unknown variant: {}", s)),
        }
    }
}

/// The `[sync]` section of the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSection {
    /// Format version (e.g., "1.2", "1.3").
    pub version: String,
    /// MIME type of the payload content.
    pub content_type: String,
    /// Display extension for the payload (e.g., "txt", "csv").
    pub display_ext: String,
    /// Sync variant (v1.3): "plain", "vault", or "app".
    #[serde(default)]
    pub variant: SyncVariant,
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

impl Default for ManifestPolicy {
    fn default() -> Self {
        Self {
            ttl: 3600,
            timeout: 30,
        }
    }
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

/// Encryption configuration (v1.3).
///
/// Controls whether the payload is encrypted and which algorithm to use.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManifestEncryption {
    /// Whether encryption is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Algorithm used (e.g., "age-v1").
    #[serde(default)]
    pub algorithm: Option<String>,
    /// UI/UX hint information (not a trusted source).
    #[serde(default)]
    pub meta: Option<EncryptionMeta>,
}

/// Encryption metadata for UI/UX hints (v1.3).
///
/// This section provides non-authoritative hints for the UI.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EncryptionMeta {
    /// Key names (e.g., `["OPENAI_API_KEY"]`).
    #[serde(default)]
    pub keys: Vec<String>,
    /// Recipients (e.g., `["did:key:z6Mk..."]`).
    #[serde(default)]
    pub recipients: Vec<String>,
    /// Key derivation function.
    #[serde(default)]
    pub kdf: Option<String>,
    /// User-facing hint message.
    #[serde(default)]
    pub hint: Option<String>,
}

/// The `[capabilities]` section of the manifest.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManifestCapabilities {
    /// Capability identifiers (e.g., "local-first", "private-network").
    #[serde(default)]
    pub values: Vec<String>,
}

/// The `[signature]` section of the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestSignature {
    /// Signature algorithm (e.g., "Ed25519").
    pub algo: String,
    /// Hash of the canonicalized manifest (e.g., "blake3:...").
    pub manifest_hash: String,
    /// Optional hash of the payload (e.g., "blake3:...").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_hash: Option<String>,
    /// RFC3339 timestamp.
    pub timestamp: String,
    /// Base64-encoded signature.
    pub value: String,
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
    /// Capabilities section.
    #[serde(default)]
    pub capabilities: ManifestCapabilities,
    /// Signature section.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<ManifestSignature>,
    /// Encryption settings (v1.3).
    #[serde(default)]
    pub encryption: ManifestEncryption,
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

    /// Check if this is a vault (encrypted) sync (v1.3).
    pub fn is_vault(&self) -> bool {
        self.sync.variant == SyncVariant::Vault || self.encryption.enabled
    }

    /// Check if this is an app capsule (v1.3).
    pub fn is_app(&self) -> bool {
        self.sync.variant == SyncVariant::App
    }

    /// Get effective timeout with default fallback (v1.3).
    pub fn timeout_secs(&self) -> u64 {
        self.policy.timeout
    }

    /// Check if write is allowed based on ownership settings.
    pub fn is_write_allowed(&self) -> bool {
        self.ownership.write_allowed
    }

    /// Check if a host is in the allowed list.
    pub fn is_host_allowed(&self, host: &str) -> bool {
        if self.permissions.allow_hosts.is_empty() {
            return false;
        }
        self.permissions
            .allow_hosts
            .iter()
            .any(|h| h == host || host.ends_with(h.trim_start_matches("*.")))
    }
}
