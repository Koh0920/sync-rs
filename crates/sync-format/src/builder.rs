use crate::{Error, Manifest, Result};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::{write::FileOptions, ZipWriter};

/// Builder for creating `.sync` archives.
#[derive(Debug, Default, Clone)]
pub struct SyncBuilder {
    manifest: Option<Manifest>,
    payload: Option<Vec<u8>>,
    context: Option<Vec<u8>>,
    wasm: Option<Vec<u8>>,
    proof: Option<Vec<u8>>,
}

impl SyncBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the manifest for the archive.
    pub fn with_manifest(mut self, manifest: Manifest) -> Self {
        self.manifest = Some(manifest);
        self
    }

    /// Set the payload as raw bytes.
    pub fn with_payload_bytes(mut self, payload: impl Into<Vec<u8>>) -> Self {
        self.payload = Some(payload.into());
        self
    }

    /// Set the context as raw bytes.
    pub fn with_context_bytes(mut self, context: impl Into<Vec<u8>>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Set the context from a JSON value.
    pub fn with_context_json(mut self, context: &serde_json::Value) -> Result<Self> {
        let data = serde_json::to_vec(context).map_err(Error::JsonError)?;
        self.context = Some(data);
        Ok(self)
    }

    /// Set the WASM module as raw bytes.
    pub fn with_wasm_bytes(mut self, wasm: impl Into<Vec<u8>>) -> Self {
        self.wasm = Some(wasm.into());
        self
    }

    /// Set the proof as raw bytes.
    pub fn with_proof_bytes(mut self, proof: impl Into<Vec<u8>>) -> Self {
        self.proof = Some(proof.into());
        self
    }

    /// Write the archive to the specified path.
    pub fn write_to<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf> {
        let manifest = self
            .manifest
            .as_ref()
            .ok_or_else(|| Error::MissingEntry("manifest.toml".to_string()))?;
        let payload = self
            .payload
            .as_ref()
            .ok_or_else(|| Error::MissingEntry("payload".to_string()))?;
        let wasm = self
            .wasm
            .as_ref()
            .ok_or_else(|| Error::MissingEntry("sync.wasm".to_string()))?;

        let manifest_text =
            toml::to_string_pretty(manifest).map_err(|e| Error::ManifestError(e.to_string()))?;

        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = File::create(path)?;
        let mut zip = ZipWriter::new(file);
        let options: FileOptions<()> =
            FileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("manifest.toml", options)?;
        zip.write_all(manifest_text.as_bytes())?;

        zip.start_file("payload", options)?;
        zip.write_all(payload)?;

        zip.start_file("sync.wasm", options)?;
        zip.write_all(wasm)?;

        if let Some(context) = &self.context {
            zip.start_file("context.json", options)?;
            zip.write_all(context)?;
        }

        if let Some(proof) = &self.proof {
            zip.start_file("sync.proof", options)?;
            zip.write_all(proof)?;
        }

        zip.finish()?;

        Ok(path.to_path_buf())
    }
}
