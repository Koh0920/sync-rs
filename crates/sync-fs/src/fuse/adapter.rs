//! FUSE adapter implementation for `.sync` archives.
//!
//! This module implements the `fuser::Filesystem` trait for `SyncFuseFS`,
//! providing zero-copy read access to archive contents.

use crate::vfs::{VfsEntry, VfsMount};
use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry,
    ReplyOpen, Request, FUSE_ROOT_ID,
};
use libc::{ENOENT, ENOTDIR};
use log::{debug, error, trace, warn};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Time-to-live for cached attributes.
const TTL: Duration = Duration::from_secs(1);

/// Block size for filesystem statistics.
const BLOCK_SIZE: u32 = 512;

/// FUSE filesystem adapter for `.sync` archives.
///
/// This struct holds the VFS metadata and provides FUSE operations
/// for mounting `.sync` archives as read-only filesystems.
pub struct SyncFuseFS {
    /// VFS metadata containing file entries.
    mount: VfsMount,
    /// Physical file handle for direct reads.
    archive_file: File,
    /// Inode to VfsEntry index mapping.
    inode_map: HashMap<u64, usize>,
    /// Name to inode reverse lookup for fast lookups.
    name_to_inode: HashMap<String, u64>,
    /// User ID for file ownership (defaults to current user).
    uid: u32,
    /// Group ID for file ownership (defaults to current group).
    gid: u32,
    /// Creation time of the filesystem.
    mount_time: SystemTime,
}

impl SyncFuseFS {
    /// Create a new FUSE filesystem from a VFS mount and archive path.
    ///
    /// # Arguments
    ///
    /// * `mount` - The VFS mount containing file metadata
    /// * `archive_path` - Path to the physical `.sync` archive file
    ///
    /// # Errors
    ///
    /// Returns an error if the archive file cannot be opened.
    pub fn new<P: AsRef<Path>>(mount: VfsMount, archive_path: P) -> io::Result<Self> {
        let file = File::open(archive_path.as_ref())?;

        let mut inode_map = HashMap::new();
        let mut name_to_inode = HashMap::new();

        // Assign inodes starting from 2 (1 is reserved for root)
        for (i, entry) in mount.entries().iter().enumerate() {
            let inode = (i + 2) as u64;
            inode_map.insert(inode, i);
            name_to_inode.insert(entry.display_name.clone(), inode);
            debug!(
                "Mapped entry '{}' to inode {} (offset={}, size={})",
                entry.display_name, inode, entry.offset, entry.size
            );
        }

        // Get current user/group IDs
        let uid = unsafe { libc::getuid() };
        let gid = unsafe { libc::getgid() };

        Ok(Self {
            mount,
            archive_file: file,
            inode_map,
            name_to_inode,
            uid,
            gid,
            mount_time: SystemTime::now(),
        })
    }

    /// Get the number of entries in this filesystem.
    pub fn entry_count(&self) -> usize {
        self.mount.entries().len()
    }

    /// Build file attributes for the root directory.
    fn root_attr(&self) -> FileAttr {
        FileAttr {
            ino: FUSE_ROOT_ID,
            size: 0,
            blocks: 0,
            atime: self.mount_time,
            mtime: self.mount_time,
            ctime: self.mount_time,
            crtime: self.mount_time,
            kind: FileType::Directory,
            perm: 0o755,
            nlink: 2,
            uid: self.uid,
            gid: self.gid,
            rdev: 0,
            blksize: BLOCK_SIZE,
            flags: 0,
        }
    }

    /// Build file attributes for a VFS entry.
    fn entry_attr(&self, inode: u64, entry: &VfsEntry) -> FileAttr {
        FileAttr {
            ino: inode,
            size: entry.size,
            blocks: (entry.size + (BLOCK_SIZE as u64) - 1) / (BLOCK_SIZE as u64),
            atime: self.mount_time,
            mtime: self.mount_time,
            ctime: self.mount_time,
            crtime: self.mount_time,
            kind: FileType::RegularFile,
            perm: if entry.read_only { 0o444 } else { 0o644 },
            nlink: 1,
            uid: self.uid,
            gid: self.gid,
            rdev: 0,
            blksize: BLOCK_SIZE,
            flags: 0,
        }
    }

    /// Get file attributes by inode.
    fn get_attr(&self, inode: u64) -> Option<FileAttr> {
        if inode == FUSE_ROOT_ID {
            return Some(self.root_attr());
        }

        let index = self.inode_map.get(&inode)?;
        let entry = &self.mount.entries()[*index];
        Some(self.entry_attr(inode, entry))
    }
}

impl Filesystem for SyncFuseFS {
    /// Get file attributes.
    fn getattr(&mut self, _req: &Request, ino: u64, _fh: Option<u64>, reply: ReplyAttr) {
        trace!("getattr(ino={})", ino);

        match self.get_attr(ino) {
            Some(attr) => reply.attr(&TTL, &attr),
            None => {
                warn!("getattr: inode {} not found", ino);
                reply.error(ENOENT);
            }
        }
    }

    /// Look up a directory entry by name.
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name_str = name.to_string_lossy();
        trace!("lookup(parent={}, name='{}')", parent, name_str);

        // Only root directory contains entries
        if parent != FUSE_ROOT_ID {
            debug!("lookup: parent {} is not root directory", parent);
            reply.error(ENOENT);
            return;
        }

        match self.name_to_inode.get(name_str.as_ref()) {
            Some(&inode) => {
                let attr = self.get_attr(inode).unwrap();
                reply.entry(&TTL, &attr, 0);
            }
            None => {
                debug!("lookup: name '{}' not found", name_str);
                reply.error(ENOENT);
            }
        }
    }

    /// Read directory entries.
    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        trace!("readdir(ino={}, offset={})", ino, offset);

        if ino != FUSE_ROOT_ID {
            warn!("readdir: inode {} is not a directory", ino);
            reply.error(ENOTDIR);
            return;
        }

        // Build entry list: ".", "..", and actual entries
        let mut entries: Vec<(u64, FileType, &str)> = vec![
            (FUSE_ROOT_ID, FileType::Directory, "."),
            (FUSE_ROOT_ID, FileType::Directory, ".."),
        ];

        for (i, entry) in self.mount.entries().iter().enumerate() {
            entries.push(((i + 2) as u64, FileType::RegularFile, &entry.display_name));
        }

        // Skip to offset and add entries until buffer is full
        for (i, (ino, kind, name)) in entries.into_iter().enumerate().skip(offset as usize) {
            // next_offset = i + 1
            let full = reply.add(ino, (i + 1) as i64, kind, name);
            if full {
                break;
            }
        }

        reply.ok();
    }

    /// Open a file.
    fn open(&mut self, _req: &Request, ino: u64, _flags: i32, reply: ReplyOpen) {
        trace!("open(ino={})", ino);

        if ino == FUSE_ROOT_ID {
            reply.error(ENOTDIR);
            return;
        }

        if !self.inode_map.contains_key(&ino) {
            reply.error(ENOENT);
            return;
        }

        // Return a dummy file handle (we use pread, no state needed)
        reply.opened(0, 0);
    }

    /// Read file data.
    ///
    /// This is the core zero-copy read implementation using `pread()`.
    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        trace!("read(ino={}, offset={}, size={})", ino, offset, size);

        // Get entry index
        let index = match self.inode_map.get(&ino) {
            Some(i) => *i,
            None => {
                warn!("read: inode {} not found", ino);
                reply.error(ENOENT);
                return;
            }
        };

        let entry = &self.mount.entries()[index];

        // Check if offset is beyond file size
        if offset < 0 || offset as u64 >= entry.size {
            trace!("read: offset {} beyond file size {}", offset, entry.size);
            reply.data(&[]);
            return;
        }

        // Calculate actual read size (clamp to remaining bytes)
        let remaining = entry.size - offset as u64;
        let read_size = std::cmp::min(size as u64, remaining) as usize;

        // Allocate buffer
        let mut buffer = vec![0u8; read_size];

        // Calculate absolute offset in the archive file
        let abs_offset = entry.offset + offset as u64;

        // Perform zero-copy read using pread
        match self.archive_file.read_at(&mut buffer, abs_offset) {
            Ok(bytes_read) => {
                trace!(
                    "read: successfully read {} bytes from offset {}",
                    bytes_read,
                    abs_offset
                );
                reply.data(&buffer[..bytes_read]);
            }
            Err(e) => {
                error!("read: I/O error reading from archive: {}", e);
                reply.error(libc::EIO);
            }
        }
    }

    /// Get filesystem statistics.
    fn statfs(&mut self, _req: &Request, _ino: u64, reply: fuser::ReplyStatfs) {
        trace!("statfs");

        // Calculate total size of all entries
        let total_size: u64 = self.mount.entries().iter().map(|e| e.size).sum();
        let blocks = (total_size + (BLOCK_SIZE as u64) - 1) / (BLOCK_SIZE as u64);
        let files = self.mount.entries().len() as u64 + 1; // +1 for root

        reply.statfs(
            blocks, // total blocks
            0,      // free blocks
            0,      // available blocks
            files,  // total inodes
            0,      // free inodes
            BLOCK_SIZE, 255, // max name length
            BLOCK_SIZE,
        );
    }
}

/// Mount a `.sync` archive as a FUSE filesystem.
///
/// This function blocks until the filesystem is unmounted.
///
/// # Arguments
///
/// * `archive_path` - Path to the `.sync` archive file
/// * `mount_point` - Directory where the archive should be mounted
/// * `vfs` - Pre-built VFS mount with file metadata
///
/// # Errors
///
/// Returns an error if:
/// - The archive file cannot be opened
/// - The mount point is invalid
/// - FUSE mounting fails
///
/// # Example
///
/// ```ignore
/// use sync_fs::fuse::mount;
/// use sync_fs::{VfsMount, VfsMountConfig};
/// use sync_format::SyncArchive;
///
/// let archive = SyncArchive::open("data.sync")?;
/// let vfs = VfsMount::from_archive(&archive, VfsMountConfig::default())?;
/// mount("data.sync", "/mnt/data", vfs)?;
/// ```
pub fn mount<P: AsRef<Path>>(archive_path: P, mount_point: P, vfs: VfsMount) -> io::Result<()> {
    let fs = SyncFuseFS::new(vfs, archive_path.as_ref())?;
    let mount_point = mount_point.as_ref();

    let options = vec![
        MountOption::RO,
        MountOption::FSName("syncfs".to_string()),
        MountOption::Subtype("sync".to_string()),
        MountOption::DefaultPermissions,
    ];

    debug!(
        "Mounting {} at {} with {} entries",
        archive_path.as_ref().display(),
        mount_point.display(),
        fs.entry_count()
    );

    fuser::mount2(fs, mount_point, &options)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("FUSE mount failed: {}", e)))
}

/// Mount a `.sync` archive in the background and return a session handle.
///
/// The filesystem will remain mounted until the returned `BackgroundSession`
/// is dropped or `unmount()` is called on it.
///
/// # Arguments
///
/// * `archive_path` - Path to the `.sync` archive file
/// * `mount_point` - Directory where the archive should be mounted
/// * `vfs` - Pre-built VFS mount with file metadata
///
/// # Returns
///
/// A `BackgroundSession` that keeps the filesystem mounted. When dropped,
/// the filesystem is automatically unmounted.
pub fn mount_background<P: AsRef<Path>>(
    archive_path: P,
    mount_point: P,
    vfs: VfsMount,
) -> io::Result<fuser::BackgroundSession> {
    let fs = SyncFuseFS::new(vfs, archive_path.as_ref())?;
    let mount_point = mount_point.as_ref();

    let options = vec![
        MountOption::RO,
        MountOption::FSName("syncfs".to_string()),
        MountOption::Subtype("sync".to_string()),
        MountOption::DefaultPermissions,
    ];

    debug!(
        "Mounting {} at {} (background) with {} entries",
        archive_path.as_ref().display(),
        mount_point.display(),
        fs.entry_count()
    );

    fuser::spawn_mount2(fs, mount_point, &options)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("FUSE mount failed: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VfsMountConfig;
    use std::path::PathBuf;
    use sync_format::SyncArchive;

    // Note: FUSE tests require macFUSE to be installed and typically
    // need to be run with elevated privileges. These tests are marked
    // as ignored by default.

    #[test]
    #[ignore = "requires macFUSE and manual testing"]
    fn test_fuse_mount() {
        // This test requires a real .sync file and mount point
        // Run manually: cargo test --features fuse test_fuse_mount -- --ignored
    }
}
