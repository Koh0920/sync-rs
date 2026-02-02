//! WebDAV filesystem implementation for `.sync` archives.
//!
//! This module implements the `dav_server::fs::DavFileSystem` trait,
//! mapping VFS entries to WebDAV resources.

use crate::vfs::{VfsEntry, VfsMount};
use dav_server::davpath::DavPath;
use dav_server::fs::{
    DavDirEntry, DavFile, DavFileSystem, DavMetaData, FsError, FsFuture, FsStream, OpenOptions,
    ReadDirMeta,
};
use futures::stream;
use log::{debug, trace};
use std::fs::File;
use std::io::SeekFrom;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

/// WebDAV filesystem adapter for `.sync` archives.
///
/// This struct implements `DavFileSystem` to expose VFS entries as WebDAV
/// resources, enabling mounting via standard OS mechanisms.
#[derive(Clone)]
pub struct SyncDavFs {
    /// VFS metadata containing file entries.
    inner: Arc<SyncDavFsInner>,
}

struct SyncDavFsInner {
    /// VFS metadata.
    mount: VfsMount,
    /// Path to the physical archive file.
    archive_path: PathBuf,
    /// Creation time for metadata.
    created: SystemTime,
}

impl SyncDavFs {
    /// Create a new WebDAV filesystem from a VFS mount.
    ///
    /// # Arguments
    ///
    /// * `mount` - The VFS mount containing file metadata
    /// * `archive_path` - Path to the physical `.sync` archive file
    pub fn new<P: Into<PathBuf>>(mount: VfsMount, archive_path: P) -> Self {
        Self {
            inner: Arc::new(SyncDavFsInner {
                mount,
                archive_path: archive_path.into(),
                created: SystemTime::now(),
            }),
        }
    }

    /// Find an entry by path.
    fn find_entry(&self, path: &DavPath) -> Option<&VfsEntry> {
        let path_str = path.as_rel_ospath().to_string_lossy();
        let name = path_str.trim_start_matches('/');

        if name.is_empty() {
            return None; // Root directory
        }

        self.inner
            .mount
            .entries()
            .iter()
            .find(|e| e.display_name == name)
    }

    /// Check if path is root.
    fn is_root(&self, path: &DavPath) -> bool {
        let path_str = path.as_rel_ospath().to_string_lossy();
        path_str == "/" || path_str.is_empty()
    }
}

impl DavFileSystem for SyncDavFs {
    fn open<'a>(&'a self, path: &'a DavPath, options: OpenOptions) -> FsFuture<'a, Box<dyn DavFile>> {
        trace!("open({:?}, {:?})", path, options);

        let result = (|| {
            // Check for write access (we're read-only)
            if options.write || options.create || options.create_new || options.truncate {
                return Err(FsError::Forbidden);
            }

            // Find the entry
            let entry = self.find_entry(path).ok_or(FsError::NotFound)?;

            // Open the archive file
            let file = File::open(&self.inner.archive_path).map_err(|_| FsError::GeneralFailure)?;

            // Create a DavFile wrapper
            let dav_file = SyncDavFile::new(entry.clone(), file);

            Ok(Box::new(dav_file) as Box<dyn DavFile>)
        })();

        Box::pin(async move { result })
    }

    fn read_dir<'a>(
        &'a self,
        path: &'a DavPath,
        meta: ReadDirMeta,
    ) -> FsFuture<'a, FsStream<Box<dyn DavDirEntry>>> {
        trace!("read_dir({:?}, {:?})", path, meta);

        let result = (|| {
            // Only root directory contains entries
            if !self.is_root(path) {
                return Err(FsError::NotFound);
            }

            // Build directory entries
            let entries: Vec<Box<dyn DavDirEntry>> = self
                .inner
                .mount
                .entries()
                .iter()
                .map(|entry| {
                    Box::new(SyncDavDirEntry::new(entry.clone(), self.inner.created))
                        as Box<dyn DavDirEntry>
                })
                .collect();

            debug!("read_dir: returning {} entries", entries.len());

            let stream = stream::iter(entries.into_iter().map(Ok));
            Ok(Box::pin(stream) as FsStream<Box<dyn DavDirEntry>>)
        })();

        Box::pin(async move { result })
    }

    fn metadata<'a>(&'a self, path: &'a DavPath) -> FsFuture<'a, Box<dyn DavMetaData>> {
        trace!("metadata({:?})", path);

        let result = (|| {
            if self.is_root(path) {
                // Root directory metadata
                Ok(Box::new(SyncDavMetaData::directory(self.inner.created))
                    as Box<dyn DavMetaData>)
            } else {
                // File metadata
                let entry = self.find_entry(path).ok_or(FsError::NotFound)?;
                Ok(Box::new(SyncDavMetaData::file(entry, self.inner.created))
                    as Box<dyn DavMetaData>)
            }
        })();

        Box::pin(async move { result })
    }
}

/// WebDAV file implementation for zero-copy reads from archive.
#[derive(Debug)]
struct SyncDavFile {
    /// VFS entry metadata.
    entry: VfsEntry,
    /// Archive file handle.
    file: File,
    /// Current read position within the entry.
    position: u64,
}

impl SyncDavFile {
    fn new(entry: VfsEntry, file: File) -> Self {
        Self {
            entry,
            file,
            position: 0,
        }
    }
}

impl DavFile for SyncDavFile {
    fn metadata(&mut self) -> FsFuture<'_, Box<dyn DavMetaData>> {
        let meta = SyncDavMetaData::file(&self.entry, SystemTime::now());
        Box::pin(async move { Ok(Box::new(meta) as Box<dyn DavMetaData>) })
    }

    fn read_bytes(&mut self, count: usize) -> FsFuture<'_, bytes::Bytes> {
        // Calculate remaining bytes
        let remaining = self.entry.size.saturating_sub(self.position);
        let to_read = std::cmp::min(count as u64, remaining) as usize;

        if to_read == 0 {
            return Box::pin(async move { Ok(bytes::Bytes::new()) });
        }

        // Calculate absolute offset
        let abs_offset = self.entry.offset + self.position;

        // Read using pread for zero-copy
        let mut buffer = vec![0u8; to_read];
        let result = self.file.read_at(&mut buffer, abs_offset);

        // Update position
        if let Ok(n) = result {
            self.position += n as u64;
        }

        Box::pin(async move {
            let n = result.map_err(|_| FsError::GeneralFailure)?;
            buffer.truncate(n);
            Ok(bytes::Bytes::from(buffer))
        })
    }

    fn seek(&mut self, pos: SeekFrom) -> FsFuture<'_, u64> {
        let new_pos = match pos {
            SeekFrom::Start(n) => n as i64,
            SeekFrom::End(n) => self.entry.size as i64 + n,
            SeekFrom::Current(n) => self.position as i64 + n,
        };

        if new_pos < 0 {
            return Box::pin(async { Err(FsError::GeneralFailure) });
        }

        self.position = new_pos as u64;
        let pos = self.position;
        Box::pin(async move { Ok(pos) })
    }

    fn write_buf(&mut self, _buf: Box<dyn bytes::Buf + Send>) -> FsFuture<'_, ()> {
        // Read-only filesystem
        Box::pin(async { Err(FsError::Forbidden) })
    }

    fn write_bytes(&mut self, _buf: bytes::Bytes) -> FsFuture<'_, ()> {
        // Read-only filesystem
        Box::pin(async { Err(FsError::Forbidden) })
    }

    fn flush(&mut self) -> FsFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}

/// WebDAV directory entry.
struct SyncDavDirEntry {
    entry: VfsEntry,
    created: SystemTime,
}

impl SyncDavDirEntry {
    fn new(entry: VfsEntry, created: SystemTime) -> Self {
        Self { entry, created }
    }
}

impl DavDirEntry for SyncDavDirEntry {
    fn name(&self) -> Vec<u8> {
        self.entry.display_name.as_bytes().to_vec()
    }

    fn metadata(&self) -> FsFuture<'_, Box<dyn DavMetaData>> {
        let meta = SyncDavMetaData::file(&self.entry, self.created);
        Box::pin(async move { Ok(Box::new(meta) as Box<dyn DavMetaData>) })
    }
}

/// WebDAV metadata for files and directories.
#[derive(Clone, Debug)]
struct SyncDavMetaData {
    is_dir: bool,
    len: u64,
    modified: SystemTime,
    created: SystemTime,
}

impl SyncDavMetaData {
    fn directory(time: SystemTime) -> Self {
        Self {
            is_dir: true,
            len: 0,
            modified: time,
            created: time,
        }
    }

    fn file(entry: &VfsEntry, time: SystemTime) -> Self {
        Self {
            is_dir: false,
            len: entry.size,
            modified: time,
            created: time,
        }
    }
}

impl DavMetaData for SyncDavMetaData {
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
