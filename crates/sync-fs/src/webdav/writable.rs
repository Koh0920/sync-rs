use crate::store::{SyncStore, SyncStoreError};
use bytes::Buf;
use dav_server::davpath::DavPath;
use dav_server::fs::{
    DavDirEntry, DavFile, DavFileSystem, DavMetaData, FsError, FsFuture, FsStream, OpenOptions,
    ReadDirMeta,
};
use futures::stream;
use log::{debug, trace, warn};
use std::fs::{self, File};
use std::io::{self, SeekFrom, Write};
use std::os::unix::fs::FileExt;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use sync_format::{Error as SyncError, SyncArchive};
use tempfile::NamedTempFile;

#[derive(Clone)]
pub struct WritableSyncFs {
    inner: Arc<WritableSyncFsInner>,
}

struct WritableSyncFsInner {
    store: Arc<SyncStore>,
    created: SystemTime,
}

impl WritableSyncFs {
    pub fn new(store: SyncStore) -> Self {
        Self {
            inner: Arc::new(WritableSyncFsInner {
                store: Arc::new(store),
                created: SystemTime::now(),
            }),
        }
    }

    pub fn store(&self) -> &SyncStore {
        &self.inner.store
    }

    fn is_root(&self, path: &DavPath) -> bool {
        let path_str = path.as_rel_ospath().to_string_lossy();
        path_str == "/" || path_str.is_empty()
    }

    fn rel_path(&self, path: &DavPath) -> Result<PathBuf, FsError> {
        let path_str = path.as_rel_ospath().to_string_lossy();
        let trimmed = path_str.trim_start_matches('/');
        if trimmed.is_empty() {
            return Ok(PathBuf::new());
        }
        let rel = Path::new(trimmed);
        for component in rel.components() {
            match component {
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(FsError::Forbidden);
                }
                _ => {}
            }
        }
        Ok(rel.to_path_buf())
    }

    fn name_from_path(&self, path: &DavPath) -> Result<String, FsError> {
        let rel = self.rel_path(path)?;
        let name = rel
            .to_string_lossy()
            .trim_start_matches('/')
            .to_string();
        if name.is_empty() {
            return Err(FsError::NotFound);
        }
        Ok(name)
    }

    fn is_ignored_name(&self, name: &str) -> bool {
        name.starts_with("._") || name == ".DS_Store"
    }

    fn resolve_sync_path(&self, name: &str) -> Result<Option<PathBuf>, FsError> {
        let candidate = self
            .inner
            .store
            .sync_path_for(name)
            .map_err(map_store_error)?;
        if candidate.exists() {
            return Ok(Some(candidate));
        }

        let rel = Path::new(name);
        let dir = rel.parent().unwrap_or_else(|| Path::new(""));
        let file_name = rel
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or(FsError::NotFound)?;
        let dir_path = self.inner.store.base_dir().join(dir);
        find_sync_in_dir(&dir_path, file_name)
    }

    fn entry_from_path(&self, sync_path: &Path) -> Result<SyncFileEntry, FsError> {
        let archive = SyncArchive::open(sync_path).map_err(map_sync_error)?;
        let payload = archive.payload_entry().ok_or(FsError::NotFound)?;

        let metadata = fs::metadata(sync_path).map_err(map_io_error)?;
        let modified = metadata.modified().unwrap_or_else(|_| SystemTime::now());
        let created = metadata.created().unwrap_or(modified);

        let base_name = archive
            .archive_file_stem()
            .unwrap_or_else(|| "payload".to_string());
        let display_name = display_name_for_archive(&base_name, &archive.manifest().sync.display_ext);

        Ok(SyncFileEntry {
            display_name,
            offset: payload.offset,
            size: payload.size,
            modified,
            created,
        })
    }

    fn entries_in_dir(&self, dir_path: &Path) -> Result<Vec<EntryInfo>, FsError> {
        let mut entries = Vec::new();
        let read_dir = fs::read_dir(dir_path).map_err(map_io_error)?;
        for entry in read_dir {
            let entry = entry.map_err(map_io_error)?;
            let name = entry.file_name().to_string_lossy().to_string();
            if self.is_ignored_name(&name) {
                continue;
            }
            let path = entry.path();
            let metadata = entry.metadata().map_err(map_io_error)?;
            if metadata.is_dir() {
                let modified = metadata.modified().unwrap_or_else(|_| SystemTime::now());
                let created = metadata.created().unwrap_or(modified);
                entries.push(EntryInfo::Directory(DirectoryEntry {
                    name,
                    modified,
                    created,
                }));
            } else if metadata.is_file() {
                if path.extension().and_then(|v| v.to_str()) != Some("sync") {
                    continue;
                }
                match self.entry_from_path(&path) {
                    Ok(mut entry) => {
                        if self.is_ignored_name(&entry.display_name) {
                            continue;
                        }
                        entry.display_name = display_name_in_dir(&entry.display_name);
                        entries.push(EntryInfo::File(entry));
                    }
                    Err(err) => {
                        warn!("Skipping invalid sync file {:?}: {:?}", path, err);
                    }
                }
            }
        }
        Ok(entries)
    }
}

impl DavFileSystem for WritableSyncFs {
    fn open<'a>(&'a self, path: &'a DavPath, options: OpenOptions) -> FsFuture<'a, Box<dyn DavFile>> {
        trace!("open({:?}, {:?})", path, options);

        let result = (|| {
            if self.is_root(path) {
                return Err(FsError::Forbidden);
            }

            let name = self.name_from_path(path)?;
            if self.is_ignored_name(&name) {
                return Ok(Box::new(NoopDavFile::new()) as Box<dyn DavFile>);
            }

            if options.append {
                return Err(FsError::NotImplemented);
            }

            let wants_write = options.write || options.create || options.create_new || options.truncate;
            if wants_write {
                let existing = self.resolve_sync_path(&name)?;
                if options.create_new && existing.is_some() {
                    return Err(FsError::Exists);
                }
                if existing.is_none() && !options.create && !options.create_new {
                    return Err(FsError::NotFound);
                }

                let buffer = if let Some(path) = existing {
                    WriteBuffer::new_existing(path)?
                } else {
                    let content_type = guess_content_type(&name).to_string();
                    WriteBuffer::new_new(name, content_type)?
                };

                let file = WritableDavFile::new_write(self.inner.store.clone(), buffer);
                return Ok(Box::new(file) as Box<dyn DavFile>);
            }

            let sync_path = self.resolve_sync_path(&name)?.ok_or(FsError::NotFound)?;
            let entry = self.entry_from_path(&sync_path)?;
            let file = File::open(&sync_path).map_err(map_io_error)?;
            let dav_file = WritableDavFile::new_read(entry, file);
            Ok(Box::new(dav_file) as Box<dyn DavFile>)
        })();

        Box::pin(async move { result })
    }

    fn read_dir<'a>(
        &'a self,
        path: &'a DavPath,
        _meta: ReadDirMeta,
    ) -> FsFuture<'a, FsStream<Box<dyn DavDirEntry>>> {
        trace!("read_dir({:?})", path);

        let result = (|| {
            let rel = self.rel_path(path)?;
            let dir_path = if rel.as_os_str().is_empty() {
                self.inner.store.base_dir().to_path_buf()
            } else {
                self.inner.store.base_dir().join(rel)
            };

            let metadata = fs::metadata(&dir_path).map_err(map_io_error)?;
            if !metadata.is_dir() {
                return Err(FsError::NotFound);
            }

            let entries = self.entries_in_dir(&dir_path)?;
            debug!("read_dir: returning {} entries", entries.len());
            let dav_entries: Vec<Box<dyn DavDirEntry>> = entries
                .into_iter()
                .map(|entry| Box::new(WritableDavDirEntry::new(entry)) as Box<dyn DavDirEntry>)
                .collect();
            let stream = stream::iter(dav_entries.into_iter().map(Ok));
            Ok(Box::pin(stream) as FsStream<Box<dyn DavDirEntry>>)
        })();

        Box::pin(async move { result })
    }

    fn metadata<'a>(&'a self, path: &'a DavPath) -> FsFuture<'a, Box<dyn DavMetaData>> {
        trace!("metadata({:?})", path);

        let result = (|| {
            if self.is_root(path) {
                return Ok(Box::new(WritableDavMetaData::directory(self.inner.created))
                    as Box<dyn DavMetaData>);
            }

            let rel = self.rel_path(path)?;
            let dir_path = self.inner.store.base_dir().join(&rel);
            if let Ok(metadata) = fs::metadata(&dir_path) {
                if metadata.is_dir() {
                    let modified = metadata.modified().unwrap_or_else(|_| SystemTime::now());
                    let created = metadata.created().unwrap_or(modified);
                    return Ok(Box::new(WritableDavMetaData::directory_with_time(
                        created, modified,
                    )) as Box<dyn DavMetaData>);
                }
            }

            let name = self.name_from_path(path)?;
            if self.is_ignored_name(&name) {
                return Ok(Box::new(WritableDavMetaData::file(0, SystemTime::now()))
                    as Box<dyn DavMetaData>);
            }

            let sync_path = self.resolve_sync_path(&name)?.ok_or(FsError::NotFound)?;
            let entry = self.entry_from_path(&sync_path)?;
            Ok(Box::new(WritableDavMetaData::file_with_time(
                entry.size,
                entry.created,
                entry.modified,
            )) as Box<dyn DavMetaData>)
        })();

        Box::pin(async move { result })
    }

    fn create_dir<'a>(&'a self, path: &'a DavPath) -> FsFuture<'a, ()> {
        trace!("create_dir({:?})", path);

        let result = (|| {
            let rel = self.rel_path(path)?;
            if rel.as_os_str().is_empty() {
                return Ok(());
            }
            let dir_path = self.inner.store.base_dir().join(rel);
            fs::create_dir_all(dir_path).map_err(map_io_error)?;
            Ok(())
        })();

        Box::pin(async move { result })
    }

    fn remove_dir<'a>(&'a self, path: &'a DavPath) -> FsFuture<'a, ()> {
        trace!("remove_dir({:?})", path);

        let result = (|| {
            let rel = self.rel_path(path)?;
            if rel.as_os_str().is_empty() {
                return Err(FsError::Forbidden);
            }
            let dir_path = self.inner.store.base_dir().join(rel);
            fs::remove_dir(dir_path).map_err(map_io_error)?;
            Ok(())
        })();

        Box::pin(async move { result })
    }

    fn remove_file<'a>(&'a self, path: &'a DavPath) -> FsFuture<'a, ()> {
        trace!("remove_file({:?})", path);

        let result = (|| {
            let name = self.name_from_path(path)?;
            if self.is_ignored_name(&name) {
                return Ok(());
            }
            let sync_path = self.resolve_sync_path(&name)?.ok_or(FsError::NotFound)?;
            self.inner
                .store
                .remove_sync(sync_path)
                .map_err(map_store_error)?;
            Ok(())
        })();

        Box::pin(async move { result })
    }

    fn rename<'a>(&'a self, from: &'a DavPath, to: &'a DavPath) -> FsFuture<'a, ()> {
        trace!("rename({:?}, {:?})", from, to);

        let result = (|| {
            let from_rel = self.rel_path(from)?;
            if from_rel.as_os_str().is_empty() {
                return Err(FsError::Forbidden);
            }
            let from_dir_path = self.inner.store.base_dir().join(&from_rel);
            if let Ok(meta) = fs::metadata(&from_dir_path) {
                if meta.is_dir() {
                    let to_rel = self.rel_path(to)?;
                    if to_rel.as_os_str().is_empty() {
                        return Err(FsError::Forbidden);
                    }
                    let to_dir_path = self.inner.store.base_dir().join(&to_rel);
                    fs::rename(from_dir_path, to_dir_path).map_err(map_io_error)?;
                    return Ok(());
                }
            }

            let from_name = self.name_from_path(from)?;
            let to_name = self.name_from_path(to)?;
            if self.is_ignored_name(&from_name) || self.is_ignored_name(&to_name) {
                return Ok(());
            }

            let from_sync_path = self.resolve_sync_path(&from_name)?.ok_or(FsError::NotFound)?;
            let to_sync_path = self
                .inner
                .store
                .sync_path_for(&to_name)
                .map_err(map_store_error)?;

            if to_sync_path.exists() {
                fs::remove_file(&to_sync_path).map_err(map_io_error)?;
            }
            if let Some(parent) = to_sync_path.parent() {
                fs::create_dir_all(parent).map_err(map_io_error)?;
            }
            fs::rename(from_sync_path, to_sync_path).map_err(map_io_error)?;
            Ok(())
        })();

        Box::pin(async move { result })
    }

    fn copy<'a>(&'a self, from: &'a DavPath, to: &'a DavPath) -> FsFuture<'a, ()> {
        trace!("copy({:?}, {:?})", from, to);

        let result = (|| {
            let from_name = self.name_from_path(from)?;
            let to_name = self.name_from_path(to)?;
            if self.is_ignored_name(&from_name) || self.is_ignored_name(&to_name) {
                return Ok(());
            }

            let from_sync_path = self.resolve_sync_path(&from_name)?.ok_or(FsError::NotFound)?;
            let to_sync_path = self
                .inner
                .store
                .sync_path_for(&to_name)
                .map_err(map_store_error)?;
            if let Some(parent) = to_sync_path.parent() {
                fs::create_dir_all(parent).map_err(map_io_error)?;
            }
            fs::copy(from_sync_path, to_sync_path).map_err(map_io_error)?;
            Ok(())
        })();

        Box::pin(async move { result })
    }
}

#[derive(Debug, Clone)]
struct SyncFileEntry {
    display_name: String,
    offset: u64,
    size: u64,
    modified: SystemTime,
    created: SystemTime,
}

#[derive(Debug, Clone)]
struct DirectoryEntry {
    name: String,
    modified: SystemTime,
    created: SystemTime,
}

#[derive(Debug, Clone)]
enum EntryInfo {
    File(SyncFileEntry),
    Directory(DirectoryEntry),
}

struct WritableDavDirEntry {
    entry: EntryInfo,
}

impl WritableDavDirEntry {
    fn new(entry: EntryInfo) -> Self {
        Self { entry }
    }
}

impl DavDirEntry for WritableDavDirEntry {
    fn name(&self) -> Vec<u8> {
        match &self.entry {
            EntryInfo::File(entry) => entry.display_name.as_bytes().to_vec(),
            EntryInfo::Directory(entry) => entry.name.as_bytes().to_vec(),
        }
    }

    fn metadata(&self) -> FsFuture<'_, Box<dyn DavMetaData>> {
        let meta = match &self.entry {
            EntryInfo::File(entry) => {
                WritableDavMetaData::file_with_time(entry.size, entry.created, entry.modified)
            }
            EntryInfo::Directory(entry) => {
                WritableDavMetaData::directory_with_time(entry.created, entry.modified)
            }
        };
        Box::pin(async move { Ok(Box::new(meta) as Box<dyn DavMetaData>) })
    }
}

#[derive(Debug)]
struct WritableDavFile {
    inner: WritableDavFileInner,
}

#[derive(Debug)]
enum WritableDavFileInner {
    Read(ReadState),
    Write(WriteState),
}

#[derive(Debug)]
struct ReadState {
    entry: SyncFileEntry,
    file: File,
    position: u64,
}

#[derive(Debug)]
struct WriteState {
    buffer: WriteBuffer,
    store: Arc<SyncStore>,
    committed: bool,
}


#[derive(Debug)]
struct WriteBuffer {
    temp: NamedTempFile,
    target: WriteTarget,
    content_type: String,
    created: SystemTime,
    modified: SystemTime,
}

#[derive(Debug)]
enum WriteTarget {
    Existing(PathBuf),
    New { name: String },
}

impl WriteBuffer {
    fn new_existing(path: PathBuf) -> Result<Self, FsError> {
        let temp = NamedTempFile::new().map_err(map_io_error)?;
        let now = SystemTime::now();
        Ok(Self {
            temp,
            target: WriteTarget::Existing(path),
            content_type: "application/octet-stream".to_string(),
            created: now,
            modified: now,
        })
    }

    fn new_new(name: String, content_type: String) -> Result<Self, FsError> {
        let temp = NamedTempFile::new().map_err(map_io_error)?;
        let now = SystemTime::now();
        Ok(Self {
            temp,
            target: WriteTarget::New { name },
            content_type,
            created: now,
            modified: now,
        })
    }

    fn len(&self) -> u64 {
        self.temp
            .as_file()
            .metadata()
            .map(|meta| meta.len())
            .unwrap_or(0)
    }
}

impl WritableDavFile {
    fn new_read(entry: SyncFileEntry, file: File) -> Self {
        Self {
            inner: WritableDavFileInner::Read(ReadState {
                entry,
                file,
                position: 0,
            }),
        }
    }

    fn new_write(store: Arc<SyncStore>, buffer: WriteBuffer) -> Self {
        Self {
            inner: WritableDavFileInner::Write(WriteState {
                buffer,
                store,
                committed: false,
            }),
        }
    }
}

impl DavFile for WritableDavFile {
    fn metadata(&mut self) -> FsFuture<'_, Box<dyn DavMetaData>> {
        let meta = match &self.inner {
            WritableDavFileInner::Read(state) => WritableDavMetaData::file_with_time(
                state.entry.size,
                state.entry.created,
                state.entry.modified,
            ),
            WritableDavFileInner::Write(state) => WritableDavMetaData::file_with_time(
                state.buffer.len(),
                state.buffer.created,
                state.buffer.modified,
            ),
        };
        Box::pin(async move { Ok(Box::new(meta) as Box<dyn DavMetaData>) })
    }

    fn read_bytes(&mut self, count: usize) -> FsFuture<'_, bytes::Bytes> {
        let result = match &mut self.inner {
            WritableDavFileInner::Read(state) => read_from_entry(state, count),
            _ => Err(FsError::Forbidden),
        };
        Box::pin(async move { result })
    }

    fn seek(&mut self, pos: SeekFrom) -> FsFuture<'_, u64> {
        let result = match &mut self.inner {
            WritableDavFileInner::Read(state) => seek_entry(state, pos),
            _ => Ok(0),
        };
        Box::pin(async move { result })
    }

    fn write_buf(&mut self, buf: Box<dyn bytes::Buf + Send>) -> FsFuture<'_, ()> {
        let result = match &mut self.inner {
            WritableDavFileInner::Write(state) => write_buf_to_temp(state, buf),
            _ => Err(FsError::Forbidden),
        };
        Box::pin(async move { result })
    }

    fn write_bytes(&mut self, buf: bytes::Bytes) -> FsFuture<'_, ()> {
        let result = match &mut self.inner {
            WritableDavFileInner::Write(state) => write_bytes_to_temp(state, &buf),
            _ => Err(FsError::Forbidden),
        };
        Box::pin(async move { result })
    }

    fn flush(&mut self) -> FsFuture<'_, ()> {
        let result = match &mut self.inner {
            WritableDavFileInner::Write(state) => commit_write(state),
            _ => Ok(()),
        };
        Box::pin(async move { result })
    }
}

#[derive(Clone, Debug)]
struct WritableDavMetaData {
    is_dir: bool,
    len: u64,
    modified: SystemTime,
    created: SystemTime,
}

impl WritableDavMetaData {
    fn directory(time: SystemTime) -> Self {
        Self {
            is_dir: true,
            len: 0,
            modified: time,
            created: time,
        }
    }

    fn directory_with_time(created: SystemTime, modified: SystemTime) -> Self {
        Self {
            is_dir: true,
            len: 0,
            modified,
            created,
        }
    }

    fn file(len: u64, time: SystemTime) -> Self {
        Self {
            is_dir: false,
            len,
            modified: time,
            created: time,
        }
    }

    fn file_with_time(len: u64, created: SystemTime, modified: SystemTime) -> Self {
        Self {
            is_dir: false,
            len,
            modified,
            created,
        }
    }
}

impl DavMetaData for WritableDavMetaData {
    fn len(&self) -> u64 {
        self.len
    }

    fn modified(&self) -> Result<SystemTime, FsError> {
        Ok(self.modified)
    }

    fn is_dir(&self) -> bool {
        self.is_dir
    }

    fn created(&self) -> Result<SystemTime, FsError> {
        Ok(self.created)
    }
}

impl NoopDavFile {
    fn new() -> Self {
        Self {
            created: SystemTime::now(),
        }
    }
}

#[derive(Debug)]
struct NoopDavFile {
    created: SystemTime,
}

impl DavFile for NoopDavFile {
    fn metadata(&mut self) -> FsFuture<'_, Box<dyn DavMetaData>> {
        let meta = WritableDavMetaData::file(0, self.created);
        Box::pin(async move { Ok(Box::new(meta) as Box<dyn DavMetaData>) })
    }

    fn write_buf(&mut self, _buf: Box<dyn bytes::Buf + Send>) -> FsFuture<'_, ()> {
        Box::pin(async move { Ok(()) })
    }

    fn write_bytes(&mut self, _buf: bytes::Bytes) -> FsFuture<'_, ()> {
        Box::pin(async move { Ok(()) })
    }

    fn read_bytes(&mut self, _count: usize) -> FsFuture<'_, bytes::Bytes> {
        Box::pin(async move { Ok(bytes::Bytes::new()) })
    }

    fn seek(&mut self, _pos: SeekFrom) -> FsFuture<'_, u64> {
        Box::pin(async move { Ok(0) })
    }

    fn flush(&mut self) -> FsFuture<'_, ()> {
        Box::pin(async move { Ok(()) })
    }
}

fn read_from_entry(state: &mut ReadState, count: usize) -> Result<bytes::Bytes, FsError> {
    let remaining = state.entry.size.saturating_sub(state.position);
    let to_read = std::cmp::min(count as u64, remaining) as usize;
    if to_read == 0 {
        return Ok(bytes::Bytes::new());
    }

    let abs_offset = state.entry.offset + state.position;
    let mut buffer = vec![0u8; to_read];
    let result = state.file.read_at(&mut buffer, abs_offset);
    if let Ok(n) = result {
        state.position += n as u64;
    }
    let n = result.map_err(|_| FsError::GeneralFailure)?;
    buffer.truncate(n);
    Ok(bytes::Bytes::from(buffer))
}

fn seek_entry(state: &mut ReadState, pos: SeekFrom) -> Result<u64, FsError> {
    let new_pos = match pos {
        SeekFrom::Start(n) => n as i64,
        SeekFrom::End(n) => state.entry.size as i64 + n,
        SeekFrom::Current(n) => state.position as i64 + n,
    };
    if new_pos < 0 {
        return Err(FsError::GeneralFailure);
    }
    state.position = new_pos as u64;
    Ok(state.position)
}

fn write_buf_to_temp(state: &mut WriteState, mut buf: Box<dyn Buf + Send>) -> Result<(), FsError> {
    if state.committed {
        return Err(FsError::Forbidden);
    }
    let file = state.buffer.temp.as_file_mut();
    while buf.has_remaining() {
        let chunk = buf.chunk();
        if chunk.is_empty() {
            break;
        }
        file.write_all(chunk).map_err(map_io_error)?;
        let len = chunk.len();
        buf.advance(len);
    }
    state.buffer.modified = SystemTime::now();
    Ok(())
}

fn write_bytes_to_temp(state: &mut WriteState, buf: &[u8]) -> Result<(), FsError> {
    if state.committed {
        return Err(FsError::Forbidden);
    }
    let file = state.buffer.temp.as_file_mut();
    file.write_all(buf).map_err(map_io_error)?;
    state.buffer.modified = SystemTime::now();
    Ok(())
}

fn commit_write(state: &mut WriteState) -> Result<(), FsError> {
    if state.committed {
        return Ok(());
    }
    state
        .buffer
        .temp
        .as_file_mut()
        .sync_all()
        .map_err(map_io_error)?;

    match &state.buffer.target {
        WriteTarget::Existing(path) => {
            state
                .store
                .update_payload_from_path(path, state.buffer.temp.path())
                .map_err(map_store_error)?;
        }
        WriteTarget::New { name } => {
            let _ = state
                .store
                .create_sync_from_path(name, state.buffer.temp.path(), &state.buffer.content_type)
                .map_err(map_store_error)?;
        }
    }

    state.committed = true;
    Ok(())
}

fn map_store_error(err: SyncStoreError) -> FsError {
    match err {
        SyncStoreError::InvalidName(_) | SyncStoreError::InvalidPath(_) => FsError::Forbidden,
        SyncStoreError::Io(err) => map_io_error(err),
        SyncStoreError::Zip(_) | SyncStoreError::Toml(_) => FsError::GeneralFailure,
    }
}

fn map_sync_error(err: SyncError) -> FsError {
    match err {
        SyncError::IoError(err) => map_io_error(err),
        _ => FsError::GeneralFailure,
    }
}

fn map_io_error(err: io::Error) -> FsError {
    match err.kind() {
        io::ErrorKind::NotFound => FsError::NotFound,
        io::ErrorKind::PermissionDenied => FsError::Forbidden,
        io::ErrorKind::AlreadyExists => FsError::Exists,
        io::ErrorKind::InvalidInput => FsError::GeneralFailure,
        _ => FsError::GeneralFailure,
    }
}

fn find_sync_in_dir(dir_path: &Path, display_name: &str) -> Result<Option<PathBuf>, FsError> {
    let read_dir = match fs::read_dir(dir_path) {
        Ok(read_dir) => read_dir,
        Err(err) => return Err(map_io_error(err)),
    };
    for entry in read_dir {
        let entry = entry.map_err(map_io_error)?;
        let path = entry.path();
        if path.extension().and_then(|v| v.to_str()) != Some("sync") {
            continue;
        }
        let archive = match SyncArchive::open(&path) {
            Ok(archive) => archive,
            Err(err) => {
                warn!("Skipping invalid sync file {:?}: {:?}", path, err);
                continue;
            }
        };
        let base_name = archive
            .archive_file_stem()
            .unwrap_or_else(|| "payload".to_string());
        let candidate = display_name_for_archive(&base_name, &archive.manifest().sync.display_ext);
        if candidate == display_name {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn display_name_for_archive(base_name: &str, display_ext: &str) -> String {
    if base_name.contains('.') {
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

fn display_name_in_dir(display_name: &str) -> String {
    Path::new(display_name)
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or(display_name)
        .to_string()
}

fn guess_content_type(name: &str) -> &'static str {
    let ext = Path::new(name)
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "txt" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "html" | "htm" => "text/html",
        "md" => "text/markdown",
        _ => "application/octet-stream",
    }
}
