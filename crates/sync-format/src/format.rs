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
}
