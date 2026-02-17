use std::fs::File;
use std::io::Write;
use sync_format::{
    Error, ManifestMetadata, ManifestPolicy, SyncArchive, SyncBuilder, SyncManifest, SyncSection,
};
use tempfile::TempDir;
use zip::{write::FileOptions, ZipWriter};

#[test]
fn sync_builder_creates_valid_archive() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("builder-test.sync");

    let manifest = SyncManifest {
        sync: SyncSection {
            version: "1.2".to_string(),
            content_type: "text/plain".to_string(),
            display_ext: "txt".to_string(),
            variant: Default::default(),
        },
        meta: ManifestMetadata {
            created_by: "builder".to_string(),
            created_at: "2099-01-23T12:00:00Z".to_string(),
            hash_algo: "blake3".to_string(),
        },
        policy: ManifestPolicy {
            ttl: 3600,
            timeout: 30,
        },
        permissions: Default::default(),
        ownership: Default::default(),
        verification: Default::default(),
        capabilities: Default::default(),
        signature: None,
        encryption: Default::default(),
    };

    SyncBuilder::new()
        .with_manifest(manifest)
        .with_payload_bytes(b"hello")
        .with_wasm_bytes(b"\0asm\x01\0\0\0")
        .write_to(&path)
        .unwrap();

    let archive = SyncArchive::open(&path).unwrap();
    assert!(archive.has_wasm());
    assert!(archive.payload_entry().is_some());
}

#[test]
fn sync_archive_rejects_compressed_payload() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("compressed-payload.sync");

    let manifest_toml = r#"
[sync]
version = "1.2"
content_type = "text/plain"
display_ext = "txt"

[meta]
created_by = "builder"
created_at = "2099-01-23T12:00:00Z"
hash_algo = "blake3"

[policy]
ttl = 3600
timeout = 30
"#;

    let file = File::create(&path).unwrap();
    let mut zip = ZipWriter::new(file);

    let stored: FileOptions<()> =
        FileOptions::default().compression_method(zip::CompressionMethod::Stored);
    #[allow(deprecated)]
    let deflated: FileOptions<()> =
        FileOptions::default().compression_method(zip::CompressionMethod::from_u16(8));
    zip.start_file("manifest.toml", stored).unwrap();
    zip.write_all(manifest_toml.as_bytes()).unwrap();

    let payload_err = zip
        .start_file("payload", deflated)
        .expect_err("compressed payload should be rejected");
    assert!(matches!(
        payload_err,
        zip::result::ZipError::UnsupportedArchive(_)
    ));
}

#[test]
fn sync_archive_missing_manifest_is_rejected() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("missing-manifest.sync");

    let file = File::create(&path).unwrap();
    let mut zip = ZipWriter::new(file);

    let options: FileOptions<()> =
        FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("payload", options).unwrap();
    zip.write_all(b"hello").unwrap();

    zip.start_file("sync.wasm", options).unwrap();
    zip.write_all(b"\0asm\x01\0\0\0").unwrap();

    zip.finish().unwrap();

    let err = SyncArchive::open(&path).expect_err("missing manifest must fail");
    assert!(matches!(err, Error::MissingEntry(entry) if entry == "manifest.toml"));
}

#[test]
fn sync_archive_missing_wasm_is_rejected() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("missing-wasm.sync");

    let manifest_toml = r#"
[sync]
version = "1.2"
content_type = "text/plain"
display_ext = "txt"

[meta]
created_by = "builder"
created_at = "2099-01-23T12:00:00Z"
hash_algo = "blake3"

[policy]
ttl = 3600
timeout = 30
"#;

    let file = File::create(&path).unwrap();
    let mut zip = ZipWriter::new(file);

    let options: FileOptions<()> =
        FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("manifest.toml", options).unwrap();
    zip.write_all(manifest_toml.as_bytes()).unwrap();

    zip.start_file("payload", options).unwrap();
    zip.write_all(b"hello").unwrap();

    zip.finish().unwrap();

    let err = SyncArchive::open(&path).expect_err("missing wasm must fail");
    assert!(matches!(err, Error::MissingEntry(entry) if entry == "sync.wasm"));
}

#[test]
fn sync_archive_detects_optional_entries() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("optional-entries.sync");

    let manifest = SyncManifest {
        sync: SyncSection {
            version: "1.2".to_string(),
            content_type: "text/plain".to_string(),
            display_ext: "txt".to_string(),
            variant: Default::default(),
        },
        meta: ManifestMetadata {
            created_by: "builder".to_string(),
            created_at: "2099-01-23T12:00:00Z".to_string(),
            hash_algo: "blake3".to_string(),
        },
        policy: ManifestPolicy {
            ttl: 3600,
            timeout: 30,
        },
        permissions: Default::default(),
        ownership: Default::default(),
        verification: Default::default(),
        capabilities: Default::default(),
        signature: None,
        encryption: Default::default(),
    };

    SyncBuilder::new()
        .with_manifest(manifest)
        .with_payload_bytes(b"hello")
        .with_wasm_bytes(b"\0asm\x01\0\0\0")
        .write_to(&path)
        .unwrap();

    let archive = SyncArchive::open(&path).unwrap();
    assert!(!archive.has_context());
    assert!(!archive.has_proof());

    let path_with_context = temp.path().join("optional-entries-with-context.sync");
    SyncBuilder::new()
        .with_manifest(archive.manifest().clone())
        .with_payload_bytes(b"hello")
        .with_wasm_bytes(b"\0asm\x01\0\0\0")
        .with_context_bytes(br#"{"value":true}"#)
        .with_proof_bytes(b"proof")
        .write_to(&path_with_context)
        .unwrap();

    let archive = SyncArchive::open(&path_with_context).unwrap();
    assert!(archive.has_context());
    assert!(archive.has_proof());
}

#[test]
fn sync_archive_update_payload_refreshes_offsets() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("update-payload.sync");

    let manifest = SyncManifest {
        sync: SyncSection {
            version: "1.2".to_string(),
            content_type: "text/plain".to_string(),
            display_ext: "txt".to_string(),
            variant: Default::default(),
        },
        meta: ManifestMetadata {
            created_by: "builder".to_string(),
            created_at: "2099-01-23T12:00:00Z".to_string(),
            hash_algo: "blake3".to_string(),
        },
        policy: ManifestPolicy {
            ttl: 3600,
            timeout: 30,
        },
        permissions: Default::default(),
        ownership: Default::default(),
        verification: Default::default(),
        capabilities: Default::default(),
        signature: None,
        encryption: Default::default(),
    };

    SyncBuilder::new()
        .with_manifest(manifest)
        .with_payload_bytes(b"hello")
        .with_wasm_bytes(b"\0asm\x01\0\0\0")
        .write_to(&path)
        .unwrap();

    let mut archive = SyncArchive::open(&path).unwrap();
    let original_offset = archive.payload_offset().unwrap();
    let original_size = archive.payload_size().unwrap();

    archive.update_payload(b"updated payload bytes").unwrap();

    let updated_offset = archive.payload_offset().unwrap();
    let updated_size = archive.payload_size().unwrap();

    assert_ne!(updated_size, original_size);
    assert_ne!(updated_offset, original_offset);
    assert!(archive.payload_entry().is_some());
}
