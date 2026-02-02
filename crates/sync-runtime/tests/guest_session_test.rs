use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use sync_runtime::{GuestAction, GuestErrorCode, GuestSession, WidgetBounds};
use tempfile::TempDir;
use zip::{write::FileOptions, ZipWriter};

fn create_test_sync_file(temp_dir: &Path) -> PathBuf {
    let manifest_toml = r#"
[sync]
version = "1.2"
content_type = "text/plain"
display_ext = "txt"

[meta]
created_by = "Capsule Sync Test"
created_at = "2099-01-23T12:00:00Z"
hash_algo = "blake3"

[policy]
ttl = 3600
timeout = 30

[permissions]
allow_hosts = ["example.com", "api.local"]
allow_env = ["FOO", "BAR"]
"#;

    let payload_data = "hello";

    let sync_path = temp_dir.join("guest-test.sync");
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
fn consumer_denies_write_payload() {
    let temp_dir = TempDir::new().unwrap();
    let sync_path = create_test_sync_file(temp_dir.path());

    let mut session = GuestSession::new(sync_path).unwrap();
    session.grant_write_payload().unwrap();

    let err = session
        .verify_permissions(&GuestAction::WritePayload)
        .expect_err("consumer should not write");

    assert!(matches!(err.code, GuestErrorCode::PermissionDenied));
}

#[test]
fn owner_allows_write_payload_when_granted() {
    let temp_dir = TempDir::new().unwrap();
    let sync_path = create_test_sync_file(temp_dir.path());

    let mut session = GuestSession::new(sync_path).unwrap();
    session.as_owner().unwrap();
    session.grant_write_payload().unwrap();

    session
        .verify_permissions(&GuestAction::WritePayload)
        .expect("owner should write when granted");
}

#[test]
fn allowlist_intersection_is_enforced() {
    let temp_dir = TempDir::new().unwrap();
    let sync_path = create_test_sync_file(temp_dir.path());

    let mut session = GuestSession::new(sync_path).unwrap();
    session.as_owner().unwrap();

    session.allow_env_var("FOO").unwrap();
    session.allow_env_var("BAZ").unwrap();
    session.allow_host("example.com").unwrap();
    session.allow_host("evil.com").unwrap();

    let context = session.get_context();

    assert_eq!(context.permissions.allowed_env, vec!["FOO".to_string()]);
    assert_eq!(
        context.permissions.allowed_hosts,
        vec!["example.com".to_string()]
    );
}

#[test]
fn widget_bounds_requires_positive_dimensions() {
    let temp_dir = TempDir::new().unwrap();
    let sync_path = create_test_sync_file(temp_dir.path());

    let mut session = GuestSession::new(sync_path).unwrap();
    let err = session
        .set_widget_bounds(WidgetBounds {
            x: 0,
            y: 0,
            width: 0,
            height: 10,
        })
        .expect_err("width 0 should be invalid");

    assert!(matches!(err.code, GuestErrorCode::InvalidRequest));
}
