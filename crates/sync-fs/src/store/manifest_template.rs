use chrono::{SecondsFormat, Utc};
use sync_format::{
    Manifest, ManifestMetadata, ManifestOwnership, ManifestPermissions, ManifestPolicy,
    ManifestVerification, SyncSection,
};

const DEFAULT_SYNC_VERSION: &str = "1.2";
const DEFAULT_HASH_ALGO: &str = "blake3";

#[derive(Debug, Clone)]
pub struct ManifestTemplate {
    pub created_by: String,
    pub default_ttl: u64,
    pub default_timeout: u64,
    pub allow_hosts: Vec<String>,
}

impl Default for ManifestTemplate {
    fn default() -> Self {
        Self {
            created_by: "sync-fs".to_string(),
            default_ttl: 3600,
            default_timeout: 30,
            allow_hosts: Vec::new(),
        }
    }
}

impl ManifestTemplate {
    pub fn to_manifest(&self, content_type: &str, display_ext: &str) -> Manifest {
        let created_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        let display_ext = normalize_display_ext(display_ext);

        Manifest {
            sync: SyncSection {
                version: DEFAULT_SYNC_VERSION.to_string(),
                content_type: content_type.to_string(),
                display_ext,
                variant: Default::default(),
            },
            meta: ManifestMetadata {
                created_by: self.created_by.clone(),
                created_at,
                hash_algo: DEFAULT_HASH_ALGO.to_string(),
            },
            policy: ManifestPolicy {
                ttl: self.default_ttl,
                timeout: self.default_timeout,
            },
            permissions: ManifestPermissions {
                allow_hosts: self.allow_hosts.clone(),
                ..Default::default()
            },
            ownership: ManifestOwnership::default(),
            verification: ManifestVerification::default(),
            capabilities: Default::default(),
            signature: None,
            encryption: Default::default(),
        }
    }
}

fn normalize_display_ext(display_ext: &str) -> String {
    let normalized = display_ext.trim().trim_start_matches('.');
    normalized.to_string()
}
