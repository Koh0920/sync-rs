use crate::manifest::SyncVariant;
use crate::{error::Result, manifest::Manifest};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Represents an entry within a `.sync` archive.
#[derive(Debug, Clone)]
pub struct SyncEntry {
    /// Name of the entry (e.g., "manifest.toml", "payload", "sync.wasm").
    pub name: String,
    /// Byte offset of the entry data within the archive.
    pub offset: u64,
    /// Uncompressed size of the entry.
    pub size: u64,
    /// Compression method used for this entry.
    pub compression: zip::CompressionMethod,
}

/// A parsed `.sync` archive.
#[derive(Debug)]
pub struct SyncArchive {
    path: String,
    entries: Vec<SyncEntry>,
    manifest: Manifest,
    payload_offset: Option<u64>,
    payload_size: Option<u64>,
}

impl SyncArchive {
    /// Open a `.sync` archive from the given path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let file = File::open(path)?;

        let mut archive = zip::ZipArchive::new(file)?;
        let mut entries = Vec::new();
        let mut manifest_data = None;
        let mut payload_offset = None;
        let mut payload_size = None;
        let mut has_wasm = false;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();
            let compression = file.compression();
            let size = file.size();

            entries.push(SyncEntry {
                name: name.clone(),
                offset: file.data_start(),
                size,
                compression,
            });

            if name == "manifest.toml" {
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                manifest_data = Some(data);
            } else if name == "sync.wasm" {
                has_wasm = true;
            } else if name == "payload" {
                if compression != zip::CompressionMethod::Stored {
                    return Err(crate::Error::InvalidFormat(
                        "payload must be stored (no compression)".to_string(),
                    ));
                }
                payload_offset = Some(file.data_start());
                payload_size = Some(size);
            }
        }

        let manifest_data =
            manifest_data.ok_or_else(|| crate::Error::MissingEntry("manifest.toml".to_string()))?;
        if !has_wasm {
            return Err(crate::Error::MissingEntry("sync.wasm".to_string()));
        }
        let manifest = Manifest::from_toml(&manifest_data)?;

        Ok(Self {
            path: path.to_string_lossy().to_string(),
            entries,
            manifest,
            payload_offset,
            payload_size,
        })
    }

    /// Get the parsed manifest.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Get all entries in the archive.
    pub fn entries(&self) -> &[SyncEntry] {
        &self.entries
    }

    /// Find an entry by name.
    pub fn entry(&self, name: &str) -> Option<&SyncEntry> {
        self.entries.iter().find(|entry| entry.name == name)
    }

    /// Get the payload entry.
    pub fn payload_entry(&self) -> Option<&SyncEntry> {
        self.entry("payload")
    }

    /// Get the byte offset of the payload within the archive.
    pub fn payload_offset(&self) -> Option<u64> {
        self.payload_offset
    }

    /// Get the size of the payload.
    pub fn payload_size(&self) -> Option<u64> {
        self.payload_size
    }

    /// Get the path to the archive file.
    pub fn archive_path(&self) -> &str {
        &self.path
    }

    /// Get the file stem (name without extension) of the archive.
    pub fn archive_file_stem(&self) -> Option<String> {
        PathBuf::from(&self.path)
            .file_stem()
            .map(|v| v.to_string_lossy().to_string())
    }

    /// Check if the archive contains a WASM module.
    pub fn has_wasm(&self) -> bool {
        self.entry("sync.wasm").is_some()
    }

    /// Check if the archive contains a proof file.
    pub fn has_proof(&self) -> bool {
        self.entry("sync.proof").is_some()
    }

    /// Check if the archive contains a context file.
    pub fn has_context(&self) -> bool {
        self.entry("context.json").is_some()
    }

    /// Update the payload in the archive with new data.
    pub fn update_payload(&mut self, new_payload: &[u8]) -> Result<()> {
        let archive_path = Path::new(&self.path);

        let mut archive = zip::ZipArchive::new(File::open(archive_path)?)?;

        let temp_path = archive_path.with_extension("sync.tmp");
        let temp_file = File::create(&temp_path)?;
        let mut temp_zip = zip::ZipWriter::new(temp_file);
        let options: zip::write::FileOptions<()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

        let entries_to_skip = HashSet::from(["payload"]);

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();

            if entries_to_skip.contains(name.as_str()) {
                continue;
            }

            temp_zip.start_file(&name, options)?;
            let mut data = Vec::new();
            file.read_to_end(&mut data)?;
            temp_zip.write_all(&data)?;
        }

        temp_zip.start_file("payload", options)?;
        temp_zip.write_all(new_payload)?;

        temp_zip.finish()?;

        fs::rename(&temp_path, archive_path)?;

        // Reload the archive
        *self = Self::open(archive_path)?;

        Ok(())
    }

    /// Check if the archive is a vault (encrypted).
    pub fn is_vault(&self) -> bool {
        self.manifest.sync.variant == SyncVariant::Vault
    }

    /// Read the payload bytes from the archive.
    pub fn read_payload(&self) -> Result<Vec<u8>> {
        let archive_path = Path::new(&self.path);
        let file = File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        let mut payload_file = archive.by_name("payload")?;
        let mut buffer = Vec::new();
        payload_file.read_to_end(&mut buffer)?;

        Ok(buffer)
    }

    /// Read the payload, decrypting if this is a vault archive.
    ///
    /// Requires the `encryption` feature.
    #[cfg(feature = "encryption")]
    pub fn read_payload_with_password(&self, password: &secrecy::SecretString) -> Result<Vec<u8>> {
        if !self.is_vault() {
            return self.read_payload();
        }

        let encrypted = self.read_payload()?;
        decrypt_data(encrypted, password)
    }

    /// Write the payload to the archive (alias for `update_payload`).
    pub fn write_payload(&mut self, new_payload: &[u8]) -> Result<()> {
        self.update_payload(new_payload)
    }

    /// Write the payload, encrypting if this is a vault archive.
    ///
    /// Requires the `encryption` feature.
    #[cfg(feature = "encryption")]
    pub fn write_payload_with_password(
        &mut self,
        data: &[u8],
        password: &secrecy::SecretString,
    ) -> Result<()> {
        let to_write = if self.is_vault() {
            encrypt_data(data, password)?
        } else {
            data.to_vec()
        };
        self.update_payload(&to_write)
    }
}

/// Encrypt data using age passphrase encryption.
///
/// Requires the `encryption` feature.
#[cfg(feature = "encryption")]
pub fn encrypt_data(data: &[u8], password: &secrecy::SecretString) -> Result<Vec<u8>> {
    use secrecy::ExposeSecret;
    let passphrase = age::Encryptor::with_user_passphrase(password.expose_secret().clone().into());
    let mut encrypted = Vec::new();
    let mut writer = passphrase
        .wrap_output(&mut encrypted)
        .map_err(|e| crate::Error::EncryptError(e.to_string()))?;
    writer.write_all(data)?;
    writer.finish()?;

    Ok(encrypted)
}

/// Decrypt data using age passphrase decryption.
///
/// Requires the `encryption` feature.
#[cfg(feature = "encryption")]
pub fn decrypt_data(encrypted: Vec<u8>, password: &secrecy::SecretString) -> Result<Vec<u8>> {
    let decryptor = match age::Decryptor::new(&encrypted[..])
        .map_err(|e| crate::Error::DecryptError(e.to_string()))?
    {
        age::Decryptor::Passphrase(d) => d,
        _ => {
            return Err(crate::Error::InvalidFormat(
                "Expected passphrase-encrypted data".to_string(),
            ));
        }
    };

    let mut decrypted = Vec::new();
    let mut reader = decryptor
        .decrypt(password, None)
        .map_err(|e| crate::Error::DecryptError(e.to_string()))?;
    reader.read_to_end(&mut decrypted)?;

    Ok(decrypted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Manifest;
    use crate::SyncBuilder;
    use tempfile::tempdir;

    fn create_test_manifest() -> Manifest {
        let toml_str = r#"
[meta]
spec_version = "1.0"
created_at = "2026-01-01T00:00:00Z"
created_by = "did:key:z6MkTest"
hash_algo = "blake3"

[sync]
version = "1.0"
content_type = "application/json"
display_ext = "json"

[policy]
ttl = 3600
timeout = 30
"#;
        Manifest::from_toml(toml_str.as_bytes()).unwrap()
    }

    fn create_minimal_wasm() -> Vec<u8> {
        // Minimal valid WASM module
        vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
    }

    #[test]
    fn test_open_valid_archive() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.sync");

        SyncBuilder::new()
            .with_manifest(create_test_manifest())
            .with_payload_bytes(b"hello world".to_vec())
            .with_wasm_bytes(create_minimal_wasm())
            .write_to(&path)
            .unwrap();

        let archive = SyncArchive::open(&path).unwrap();
        assert_eq!(archive.manifest().sync.content_type, "application/json");
        assert!(archive.has_wasm());
    }

    #[test]
    fn test_missing_manifest() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("no-manifest.zip");

        // Create a zip without manifest.toml
        let file = File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        zip.start_file("sync.wasm", options).unwrap();
        zip.write_all(&create_minimal_wasm()).unwrap();
        zip.finish().unwrap();

        let result = SyncArchive::open(&path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("manifest.toml"));
    }

    #[test]
    fn test_missing_wasm() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("no-wasm.zip");

        // Create a zip without sync.wasm
        let manifest = create_test_manifest();
        let manifest_toml = toml::to_string_pretty(&manifest).unwrap();

        let file = File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        zip.start_file("manifest.toml", options).unwrap();
        zip.write_all(manifest_toml.as_bytes()).unwrap();
        zip.finish().unwrap();

        let result = SyncArchive::open(&path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("sync.wasm"));
    }

    #[test]
    fn test_entry_lookup() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("lookup.sync");

        SyncBuilder::new()
            .with_manifest(create_test_manifest())
            .with_payload_bytes(b"hello".to_vec())
            .with_wasm_bytes(create_minimal_wasm())
            .write_to(&path)
            .unwrap();

        let archive = SyncArchive::open(&path).unwrap();

        assert!(archive.entry("manifest.toml").is_some());
        assert!(archive.entry("sync.wasm").is_some());
        assert!(archive.entry("payload").is_some());
        assert!(archive.entry("nonexistent").is_none());
    }

    #[test]
    fn test_payload_offset_and_size() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("payload-meta.sync");
        let payload = b"hello world payload data".to_vec();
        let payload_len = payload.len();

        SyncBuilder::new()
            .with_manifest(create_test_manifest())
            .with_payload_bytes(payload)
            .with_wasm_bytes(create_minimal_wasm())
            .write_to(&path)
            .unwrap();

        let archive = SyncArchive::open(&path).unwrap();

        assert!(archive.payload_offset().is_some());
        assert_eq!(archive.payload_size(), Some(payload_len as u64));
    }

    #[test]
    fn test_update_payload() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("update.sync");

        SyncBuilder::new()
            .with_manifest(create_test_manifest())
            .with_payload_bytes(b"original data".to_vec())
            .with_wasm_bytes(create_minimal_wasm())
            .write_to(&path)
            .unwrap();

        let mut archive = SyncArchive::open(&path).unwrap();
        let new_payload = b"updated data with more content";
        archive.update_payload(new_payload).unwrap();

        // Verify update
        assert_eq!(archive.payload_size(), Some(new_payload.len() as u64));

        // Reopen and verify
        let archive2 = SyncArchive::open(&path).unwrap();
        assert_eq!(archive2.payload_size(), Some(new_payload.len() as u64));
        assert!(archive2.has_wasm()); // WASM should still exist
    }

    #[test]
    fn test_has_context_and_proof() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("context-proof.sync");

        // Create archive without context/proof
        SyncBuilder::new()
            .with_manifest(create_test_manifest())
            .with_payload_bytes(b"data".to_vec())
            .with_wasm_bytes(create_minimal_wasm())
            .write_to(&path)
            .unwrap();

        let archive = SyncArchive::open(&path).unwrap();
        assert!(!archive.has_context());
        assert!(!archive.has_proof());
    }

    #[test]
    fn test_archive_file_stem() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("my-data-file.sync");

        SyncBuilder::new()
            .with_manifest(create_test_manifest())
            .with_payload_bytes(b"x".to_vec())
            .with_wasm_bytes(create_minimal_wasm())
            .write_to(&path)
            .unwrap();

        let archive = SyncArchive::open(&path).unwrap();
        assert_eq!(
            archive.archive_file_stem(),
            Some("my-data-file".to_string())
        );
    }
}
