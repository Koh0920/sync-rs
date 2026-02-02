use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use sync_format::SyncArchive;
use sync_fs::{VfsMount, VfsMountConfig};
use tempfile::TempDir;
use zip::{write::FileOptions, ZipWriter};

fn create_test_sync_file(temp_dir: &Path) -> PathBuf {
    let manifest_toml = r#"
[sync]
version = "1.2"
content_type = "text/csv"
display_ext = "csv"

[meta]
created_by = "Capsule Sync Test"
created_at = "2099-01-23T12:00:00Z"
hash_algo = "blake3"

[policy]
ttl = 3600
timeout = 30
"#;

    let payload_data = "hello";

    let sync_path = temp_dir.join("report.csv.sync");
    let file = File::create(&sync_path).unwrap();
    let mut zip = ZipWriter::new(file);

    let options: FileOptions<()> =
        FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("manifest.toml", options).unwrap();
    zip.write_all(manifest_toml.as_bytes()).unwrap();

    zip.start_file("payload", options).unwrap();
    zip.write_all(payload_data.as_bytes()).unwrap();

    zip.start_file("sync.wasm", options).unwrap();
    zip.write_all(b"\0asm\x01\0\0\0").unwrap();

    zip.finish().unwrap();

    sync_path
}

#[test]
fn vfs_display_name_and_path_are_computed() {
    let temp_dir = TempDir::new().unwrap();
    let sync_path = create_test_sync_file(temp_dir.path());

    let archive = SyncArchive::open(&sync_path).unwrap();
    let config = VfsMountConfig {
        mount_path: PathBuf::from("/mnt"),
        expose_as_read_only: true,
        show_original_extension: true,
    };
    let mount = VfsMount::from_archive(&archive, config).unwrap();
    let entry = mount.get_payload_entry().unwrap();

    assert_eq!(entry.display_name, "report.csv");
    assert_eq!(entry.vfs_path, PathBuf::from("/mnt/report.csv"));
}
