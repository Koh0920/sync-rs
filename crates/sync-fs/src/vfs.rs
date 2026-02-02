use std::path::PathBuf;
use sync_format::SyncArchive;

/// Represents a virtual file entry mapped from a `.sync` archive.
#[derive(Debug, Clone)]
pub struct VfsEntry {
    /// Internal name of the entry (e.g., "payload").
    pub name: String,
    /// User-visible display name with extension.
    pub display_name: String,
    /// MIME type of the content.
    pub content_type: String,
    /// Path to the source `.sync` file.
    pub file_path: PathBuf,
    /// Virtual path where this entry appears.
    pub vfs_path: PathBuf,
    /// Byte offset within the archive.
    pub offset: u64,
    /// Size in bytes.
    pub size: u64,
    /// Whether the entry is read-only.
    pub read_only: bool,
}

/// Configuration for VFS mounting.
#[derive(Debug, Clone)]
pub struct VfsMountConfig {
    /// Base path for mounting virtual entries.
    pub mount_path: PathBuf,
    /// Whether to expose entries as read-only.
    pub expose_as_read_only: bool,
    /// Whether to append the original extension to display names.
    pub show_original_extension: bool,
}

impl Default for VfsMountConfig {
    fn default() -> Self {
        Self {
            mount_path: PathBuf::from("/"),
            expose_as_read_only: true,
            show_original_extension: true,
        }
    }
}

/// A virtual filesystem mount from a `.sync` archive.
#[derive(Debug)]
pub struct VfsMount {
    config: VfsMountConfig,
    entries: Vec<VfsEntry>,
}

impl VfsMount {
    /// Create a new empty VFS mount with the given configuration.
    pub fn new(config: VfsMountConfig) -> Self {
        Self {
            config,
            entries: Vec::new(),
        }
    }

    /// Create a VFS mount from an archive, automatically adding the payload.
    pub fn from_archive(
        archive: &SyncArchive,
        config: VfsMountConfig,
    ) -> sync_format::Result<Self> {
        let mut mount = Self::new(config);
        mount.add_payload_from_archive(archive)?;
        Ok(mount)
    }

    /// Add the payload entry from an archive to this mount.
    pub fn add_payload_from_archive(&mut self, archive: &SyncArchive) -> sync_format::Result<()> {
        let payload_entry = archive
            .payload_entry()
            .ok_or(sync_format::Error::PayloadNotFound)?;
        let manifest = archive.manifest();

        let base_name = archive
            .archive_file_stem()
            .unwrap_or_else(|| "payload".to_string());
        let display_name = build_display_name(
            &base_name,
            &manifest.sync.display_ext,
            self.config.show_original_extension,
        );
        let vfs_path = self.config.mount_path.join(&display_name);

        let entry = VfsEntry {
            name: "payload".to_string(),
            display_name,
            content_type: manifest.sync.content_type.clone(),
            file_path: PathBuf::from(archive.archive_path()),
            vfs_path,
            offset: payload_entry.offset,
            size: payload_entry.size,
            read_only: self.config.expose_as_read_only,
        };

        self.entries.push(entry);
        Ok(())
    }

    /// Get all virtual file entries.
    pub fn entries(&self) -> &[VfsEntry] {
        &self.entries
    }

    /// Get the mount configuration.
    pub fn config(&self) -> &VfsMountConfig {
        &self.config
    }

    /// Get the payload entry if present.
    pub fn get_payload_entry(&self) -> Option<&VfsEntry> {
        self.entries.iter().find(|e| e.name == "payload")
    }
}

fn build_display_name(base_name: &str, display_ext: &str, show_original_extension: bool) -> String {
    if !show_original_extension {
        return base_name.to_string();
    }

    let normalized = display_ext.trim().trim_start_matches('.');
    if normalized.is_empty() {
        return base_name.to_string();
    }

    let suffix = format!(".{}", normalized);
    if base_name.ends_with(&suffix) {
        base_name.to_string()
    } else {
        format!("{}{}", base_name, suffix)
    }
}
