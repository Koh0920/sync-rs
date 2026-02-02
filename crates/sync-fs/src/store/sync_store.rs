use super::ManifestTemplate;
use std::fs::{self, File};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use tempfile::NamedTempFile;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

#[derive(Debug, thiserror::Error)]
pub enum SyncStoreError {
    #[error("invalid sync name: {0}")]
    InvalidName(String),
    #[error("invalid relative path: {0}")]
    InvalidPath(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Toml(#[from] toml::ser::Error),
}

pub type SyncStoreResult<T> = Result<T, SyncStoreError>;

#[derive(Debug, Clone)]
pub struct SyncStore {
    base_dir: PathBuf,
    manifest_template: ManifestTemplate,
    minimal_wasm: Vec<u8>,
}

impl SyncStore {
    pub fn new<P: Into<PathBuf>>(base_dir: P) -> Self {
        Self::with_template(base_dir, ManifestTemplate::default())
    }

    pub fn with_template<P: Into<PathBuf>>(
        base_dir: P,
        manifest_template: ManifestTemplate,
    ) -> Self {
        Self {
            base_dir: base_dir.into(),
            manifest_template,
            minimal_wasm: b"\0asm\x01\0\0\0".to_vec(),
        }
    }

    pub fn with_minimal_wasm(mut self, minimal_wasm: Vec<u8>) -> Self {
        self.minimal_wasm = minimal_wasm;
        self
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn manifest_template(&self) -> &ManifestTemplate {
        &self.manifest_template
    }

    pub fn create_sync_from_path<P: AsRef<Path>>(
        &self,
        name: &str,
        payload_path: P,
        content_type: &str,
    ) -> SyncStoreResult<PathBuf> {
        let sync_path = self.sync_path_for(name)?;
        let parent = sync_path
            .parent()
            .ok_or_else(|| SyncStoreError::InvalidPath(sync_path.display().to_string()))?;
        fs::create_dir_all(parent)?;

        let display_ext = display_ext_from_name(name);
        let manifest = self
            .manifest_template
            .to_manifest(content_type, &display_ext);
        let manifest_text = toml::to_string_pretty(&manifest)?;

        let mut payload_file = File::open(payload_path.as_ref())?;
        payload_file.seek(SeekFrom::Start(0))?;

        let mut temp = tempfile::Builder::new()
            .prefix(".tmp.sync-")
            .suffix(".sync")
            .tempfile_in(parent)?;

        write_new_archive(
            temp.as_file_mut(),
            &manifest_text,
            &mut payload_file,
            &self.minimal_wasm,
        )?;

        persist_tempfile(temp, &sync_path)?;

        Ok(sync_path)
    }

    pub fn update_payload_from_path<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        sync_path: P,
        payload_path: Q,
    ) -> SyncStoreResult<()> {
        let sync_path = self.resolve_sync_path(sync_path.as_ref())?;
        let parent = sync_path
            .parent()
            .ok_or_else(|| SyncStoreError::InvalidPath(sync_path.display().to_string()))?;

        let mut payload_file = File::open(payload_path.as_ref())?;
        payload_file.seek(SeekFrom::Start(0))?;

        let mut archive = ZipArchive::new(File::open(&sync_path)?)?;
        let mut temp = tempfile::Builder::new()
            .prefix(".tmp.sync-")
            .suffix(".sync")
            .tempfile_in(parent)?;

        write_updated_archive(temp.as_file_mut(), &mut archive, &mut payload_file)?;
        persist_tempfile(temp, &sync_path)?;

        Ok(())
    }

    pub fn remove_sync<P: AsRef<Path>>(&self, sync_path: P) -> SyncStoreResult<()> {
        let sync_path = self.resolve_sync_path(sync_path.as_ref())?;
        if sync_path.exists() {
            fs::remove_file(sync_path)?;
        }
        Ok(())
    }

    pub fn list_syncs(&self) -> SyncStoreResult<Vec<PathBuf>> {
        let mut entries = Vec::new();
        if !self.base_dir.exists() {
            return Ok(entries);
        }
        collect_syncs(&self.base_dir, &mut entries)?;
        Ok(entries)
    }

    pub fn sync_path_for(&self, name: &str) -> SyncStoreResult<PathBuf> {
        let rel = normalize_relative_path(name)?;
        let mut rel_sync = rel;
        let file_name = rel_sync
            .file_name()
            .ok_or_else(|| SyncStoreError::InvalidName(name.to_string()))?;
        let sync_name = ensure_sync_extension(&file_name.to_string_lossy());
        rel_sync.set_file_name(sync_name);
        Ok(self.base_dir.join(rel_sync))
    }

    fn resolve_sync_path(&self, path: &Path) -> SyncStoreResult<PathBuf> {
        if path.is_absolute() {
            return Ok(path.to_path_buf());
        }

        let rel = normalize_relative_path(&path.to_string_lossy())?;
        Ok(self.base_dir.join(rel))
    }
}

fn write_new_archive<W: Write + Seek>(
    writer: &mut W,
    manifest_text: &str,
    payload: &mut File,
    minimal_wasm: &[u8],
) -> SyncStoreResult<()> {
    let mut zip = ZipWriter::new(writer);
    let options: FileOptions<()> =
        FileOptions::default().compression_method(CompressionMethod::Stored);

    zip.start_file("manifest.toml", options)?;
    zip.write_all(manifest_text.as_bytes())?;

    zip.start_file("payload", options)?;
    io::copy(payload, &mut zip)?;

    zip.start_file("sync.wasm", options)?;
    zip.write_all(minimal_wasm)?;

    zip.finish()?;
    Ok(())
}

fn write_updated_archive<W: Write + Seek>(
    writer: &mut W,
    archive: &mut ZipArchive<File>,
    payload: &mut File,
) -> SyncStoreResult<()> {
    let mut zip = ZipWriter::new(writer);

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        if name == "payload" {
            continue;
        }

        let options: FileOptions<()> =
            FileOptions::default().compression_method(file.compression());
        zip.start_file(&name, options)?;
        io::copy(&mut file, &mut zip)?;
    }

    let payload_options: FileOptions<()> =
        FileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("payload", payload_options)?;
    io::copy(payload, &mut zip)?;

    zip.finish()?;
    Ok(())
}

fn persist_tempfile(temp: NamedTempFile, final_path: &Path) -> SyncStoreResult<()> {
    temp.persist(final_path)
        .map(|_| ())
        .map_err(|err| SyncStoreError::Io(err.error))
}

fn normalize_relative_path(path: &str) -> SyncStoreResult<PathBuf> {
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() {
        return Err(SyncStoreError::InvalidName(path.to_string()));
    }

    let rel = Path::new(trimmed);
    for component in rel.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(SyncStoreError::InvalidPath(path.to_string()));
            }
            _ => {}
        }
    }

    Ok(rel.to_path_buf())
}

fn ensure_sync_extension(name: &str) -> String {
    if name.ends_with(".sync") {
        name.to_string()
    } else {
        format!("{}.sync", name)
    }
}

fn display_ext_from_name(name: &str) -> String {
    let trimmed = name.trim_start_matches('/');
    let without_sync = trimmed.strip_suffix(".sync").unwrap_or(trimmed);
    Path::new(without_sync)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_string()
}

fn collect_syncs(dir: &Path, entries: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_syncs(&path, entries)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("sync") {
            entries.push(path);
        }
    }
    Ok(())
}
