//! Signature verification for `.sync` files.
//!
//! This module provides Ed25519 signature verification using BLAKE3 hashes.
//! See docs/SIGNATURE_SPEC.md for the full specification.

use crate::manifest::SyncManifest;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;

/// Signature format for Ato AppSync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSignature {
    /// Signature algorithm (always "Ed25519")
    pub algorithm: String,
    /// Signer's public key in did:key format
    pub public_key: String,
    /// Base64-encoded Ed25519 signature
    pub signature: String,
    /// BLAKE3 hash of the .sync file (format: "blake3:<64-char-hex>")
    pub content_hash: String,
    /// Unix timestamp (seconds since epoch)
    pub signed_at: u64,
}

/// Verification result with details
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Whether the signature is valid
    pub valid: bool,
    /// Extracted public key bytes (if valid)
    pub public_key_bytes: Option<[u8; 32]>,
    /// Computed BLAKE3 hash
    pub computed_hash: String,
    /// Error message (if invalid)
    pub error: Option<String>,
}

/// Manifest signature verification result
#[derive(Debug, Clone)]
pub struct ManifestSignatureResult {
    /// Whether the signature is valid
    pub valid: bool,
    /// Computed manifest hash
    pub manifest_hash: String,
    /// Computed payload hash
    pub payload_hash: Option<String>,
    /// Error message (if invalid)
    pub error: Option<String>,
}

/// Errors that can occur during verification
#[derive(Debug, thiserror::Error)]
pub enum VerificationError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid signature format: {0}")]
    InvalidFormat(String),
    #[error("Invalid DID format: {0}")]
    InvalidDid(String),
    #[error("Invalid public key: {0}")]
    InvalidPublicKey(String),
    #[error("Invalid signature: {0}")]
    InvalidSignature(String),
    #[error("Content hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("Unsupported algorithm: {0}")]
    UnsupportedAlgorithm(String),
}

/// Verify a `.sync` file against a signature
///
/// # Arguments
///
/// * `path` - Path to the `.sync` file
/// * `signature` - The signature to verify
///
/// # Returns
///
/// Returns `Ok(VerificationResult)` if verification completed (valid or invalid).
/// Returns `Err(VerificationError)` if an error occurred during verification.
///
/// # Example
///
/// ```ignore
/// use sync_format::verification::{verify_sync_file, SyncSignature};
///
/// let signature: SyncSignature = serde_json::from_str(sig_json)?;
/// let result = verify_sync_file("app.sync", &signature)?;
///
/// if result.valid {
///     println!("Signature verified! Author: {}", signature.public_key);
/// }
/// ```
pub fn verify_sync_file(
    path: &Path,
    signature: &SyncSignature,
) -> Result<VerificationResult, VerificationError> {
    // Step 1: Verify algorithm
    if signature.algorithm != "Ed25519" {
        return Err(VerificationError::UnsupportedAlgorithm(
            signature.algorithm.clone(),
        ));
    }

    // Step 2: Read file and compute BLAKE3 hash
    let bytes = std::fs::read(path)?;
    let hash = blake3::hash(&bytes);
    let computed_hash = format!("blake3:{}", hex::encode(hash.as_bytes()));

    // Step 3: Verify content hash matches
    if computed_hash != signature.content_hash {
        return Ok(VerificationResult {
            valid: false,
            public_key_bytes: None,
            computed_hash: computed_hash.clone(),
            error: Some(format!(
                "Content hash mismatch: expected {}, got {}",
                signature.content_hash, computed_hash
            )),
        });
    }

    // Step 4: Extract public key from did:key
    let public_key_bytes = match extract_public_key(&signature.public_key) {
        Ok(bytes) => bytes,
        Err(e) => {
            return Ok(VerificationResult {
                valid: false,
                public_key_bytes: None,
                computed_hash: computed_hash.clone(),
                error: Some(format!("Invalid public key: {}", e)),
            });
        }
    };

    // Step 5: Verify Ed25519 signature
    let verifying_key = match VerifyingKey::from_bytes(&public_key_bytes) {
        Ok(key) => key,
        Err(e) => {
            return Ok(VerificationResult {
                valid: false,
                public_key_bytes: Some(public_key_bytes),
                computed_hash: computed_hash.clone(),
                error: Some(format!("Invalid verifying key: {}", e)),
            });
        }
    };

    let signature_bytes = match base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &signature.signature,
    ) {
        Ok(bytes) => bytes,
        Err(e) => {
            return Ok(VerificationResult {
                valid: false,
                public_key_bytes: Some(public_key_bytes),
                computed_hash: computed_hash.clone(),
                error: Some(format!("Invalid base64 signature: {}", e)),
            });
        }
    };

    if signature_bytes.len() != 64 {
        return Ok(VerificationResult {
            valid: false,
            public_key_bytes: Some(public_key_bytes),
            computed_hash: computed_hash.clone(),
            error: Some(format!(
                "Invalid signature length: expected 64, got {}",
                signature_bytes.len()
            )),
        });
    }

    let sig_array: [u8; 64] = match signature_bytes.try_into() {
        Ok(arr) => arr,
        Err(_) => {
            return Ok(VerificationResult {
                valid: false,
                public_key_bytes: Some(public_key_bytes),
                computed_hash: computed_hash.clone(),
                error: Some("Failed to convert signature to array".to_string()),
            });
        }
    };

    let ed_sig = Signature::from_bytes(&sig_array);

    match verifying_key.verify(hash.as_bytes(), &ed_sig) {
        Ok(_) => Ok(VerificationResult {
            valid: true,
            public_key_bytes: Some(public_key_bytes),
            computed_hash,
            error: None,
        }),
        Err(e) => Ok(VerificationResult {
            valid: false,
            public_key_bytes: Some(public_key_bytes),
            computed_hash,
            error: Some(format!("Signature verification failed: {}", e)),
        }),
    }
}

/// Verify a `.sync` file using the manifest `[signature]` section
pub fn verify_manifest_signature(
    path: &Path,
) -> Result<ManifestSignatureResult, VerificationError> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| VerificationError::InvalidFormat(format!("Invalid sync archive: {e}")))?;

    let mut manifest_text = String::new();
    archive
        .by_name("manifest.toml")
        .map_err(|e| VerificationError::InvalidFormat(format!("Missing manifest.toml: {e}")))?
        .read_to_string(&mut manifest_text)
        .map_err(|e| {
            VerificationError::InvalidFormat(format!("Failed to read manifest.toml: {e}"))
        })?;

    let mut manifest: SyncManifest = SyncManifest::from_toml(manifest_text.as_bytes())
        .map_err(|e| VerificationError::InvalidFormat(format!("Invalid manifest.toml: {e}")))?;

    let signature = match manifest.signature.clone() {
        Some(sig) => sig,
        None => {
            return Ok(ManifestSignatureResult {
                valid: false,
                manifest_hash: String::new(),
                payload_hash: None,
                error: Some("signature section missing".to_string()),
            })
        }
    };

    if signature.algo != "Ed25519" {
        return Err(VerificationError::UnsupportedAlgorithm(signature.algo));
    }

    // Compute manifest hash (exclude signature)
    manifest.signature = None;
    let manifest_json = serde_json::to_value(&manifest).map_err(|e| {
        VerificationError::InvalidFormat(format!("Manifest JSON encode failed: {e}"))
    })?;
    let canonical_manifest = canonicalize_json(&manifest_json);
    let manifest_bytes = serde_json::to_vec(&canonical_manifest).map_err(|e| {
        VerificationError::InvalidFormat(format!("Manifest JSON serialize failed: {e}"))
    })?;
    let manifest_hash = format!(
        "blake3:{}",
        hex::encode(blake3::hash(&manifest_bytes).as_bytes())
    );

    if manifest_hash != signature.manifest_hash {
        return Ok(ManifestSignatureResult {
            valid: false,
            manifest_hash,
            payload_hash: None,
            error: Some("manifest hash mismatch".to_string()),
        });
    }

    // Compute payload hash if provided
    let mut payload_hash = None;
    if signature.payload_hash.is_some() {
        let mut payload_bytes = Vec::new();
        archive
            .by_name("payload")
            .map_err(|e| VerificationError::InvalidFormat(format!("Missing payload: {e}")))?
            .read_to_end(&mut payload_bytes)
            .map_err(|e| {
                VerificationError::InvalidFormat(format!("Failed to read payload: {e}"))
            })?;
        let computed = format!(
            "blake3:{}",
            hex::encode(blake3::hash(&payload_bytes).as_bytes())
        );
        payload_hash = Some(computed.clone());
        if Some(computed) != signature.payload_hash {
            return Ok(ManifestSignatureResult {
                valid: false,
                manifest_hash,
                payload_hash,
                error: Some("payload hash mismatch".to_string()),
            });
        }
    }

    // Build signing payload
    let signer = manifest.meta.created_by.clone();
    let signing_payload = serde_json::json!({
        "manifest_hash": signature.manifest_hash,
        "payload_hash": signature.payload_hash,
        "timestamp": signature.timestamp,
        "signer": signer,
    });
    let canonical_payload = canonicalize_json(&signing_payload);
    let payload_bytes = serde_json::to_vec(&canonical_payload).map_err(|e| {
        VerificationError::InvalidFormat(format!("Signing payload serialize failed: {e}"))
    })?;

    let public_key_bytes = extract_public_key(&manifest.meta.created_by)
        .map_err(|e| VerificationError::InvalidPublicKey(format!("{e}")))?;
    let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)
        .map_err(|e| VerificationError::InvalidPublicKey(format!("{e}")))?;
    let signature_bytes =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &signature.value)
            .map_err(|e| {
                VerificationError::InvalidSignature(format!("Invalid base64 signature: {e}"))
            })?;

    if signature_bytes.len() != 64 {
        return Ok(ManifestSignatureResult {
            valid: false,
            manifest_hash,
            payload_hash,
            error: Some("Invalid signature length".to_string()),
        });
    }

    let sig_array: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| VerificationError::InvalidSignature("Invalid signature length".to_string()))?;
    let ed_sig = Signature::from_bytes(&sig_array);

    match verifying_key.verify(&payload_bytes, &ed_sig) {
        Ok(_) => Ok(ManifestSignatureResult {
            valid: true,
            manifest_hash,
            payload_hash,
            error: None,
        }),
        Err(e) => Ok(ManifestSignatureResult {
            valid: false,
            manifest_hash,
            payload_hash,
            error: Some(format!("Signature verification failed: {e}")),
        }),
    }
}

/// Extract public key bytes from did:key format
///
/// Format: did:key:z6Mk<multibase-base58btc-ed25519-public-key>
/// Multicodec prefix: 0xed01 (Ed25519 public key)
fn extract_public_key(did: &str) -> Result<[u8; 32], VerificationError> {
    // Check prefix
    if !did.starts_with("did:key:z") {
        return Err(VerificationError::InvalidDid(
            "DID must start with 'did:key:z'".to_string(),
        ));
    }

    // Extract encoded part
    let encoded = &did[9..]; // Remove "did:key:z"

    // Decode base58btc
    let decoded = bs58::decode(encoded)
        .into_vec()
        .map_err(|e| VerificationError::InvalidDid(format!("Base58 decode failed: {}", e)))?;

    // Parse multicodec prefix (0xed01 for Ed25519)
    if decoded.len() < 2 {
        return Err(VerificationError::InvalidDid(
            "Decoded data too short".to_string(),
        ));
    }

    // Check multicodec prefix manually (0xed01 = [0xed, 0x01])
    if decoded[0] != 0xed || decoded[1] != 0x01 {
        return Err(VerificationError::InvalidDid(format!(
            "Invalid multicodec prefix: expected 0xed01, got 0x{:02x}{:02x}",
            decoded[0], decoded[1]
        )));
    }

    // Extract key bytes (remaining bytes after 2-byte prefix)
    let key_bytes = &decoded[2..];

    if key_bytes.len() != 32 {
        return Err(VerificationError::InvalidPublicKey(format!(
            "Invalid key length: expected 32, got {}",
            key_bytes.len()
        )));
    }

    let mut result = [0u8; 32];
    result.copy_from_slice(key_bytes);
    Ok(result)
}

fn canonicalize_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut sorted = std::collections::BTreeMap::new();
            for (key, val) in map {
                sorted.insert(key.clone(), canonicalize_json(val));
            }
            let mut new_map = serde_json::Map::new();
            for (key, val) in sorted {
                new_map.insert(key, val);
            }
            serde_json::Value::Object(new_map)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(canonicalize_json).collect())
        }
        _ => value.clone(),
    }
}

/// Compute BLAKE3 hash of a `.sync` file
///
/// Returns the hash in format "blake3:<64-char-hex>"
pub fn compute_content_hash(path: &Path) -> Result<String, std::io::Error> {
    let bytes = std::fs::read(path)?;
    let hash = blake3::hash(&bytes);
    Ok(format!("blake3:{}", hex::encode(hash.as_bytes())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_public_key_valid() {
        // This is a test DID - not a real key
        // did:key:z6MkhaXg... format with proper multicodec prefix
        // The actual key would be derived from a real Ed25519 key
        let did = "did:key:z6MkhaXgEzG8b4P8Xn8Xn8Xn8Xn8Xn8Xn8Xn8Xn8Xn8Xn8";

        // This will fail because it's not a valid base58 string
        // but we test the format validation
        let result = extract_public_key(did);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_public_key_invalid_prefix() {
        let did = "did:web:example.com";
        let result = extract_public_key(did);
        assert!(matches!(result, Err(VerificationError::InvalidDid(_))));
    }

    #[test]
    fn test_sync_signature_serialization() {
        let sig = SyncSignature {
            algorithm: "Ed25519".to_string(),
            public_key: "did:key:z6Mktest".to_string(),
            signature: "dGVzdA==".to_string(),
            content_hash: "blake3:a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6"
                .to_string(),
            signed_at: 1234567890,
        };

        let json = serde_json::to_string(&sig).unwrap();
        let deserialized: SyncSignature = serde_json::from_str(&json).unwrap();

        assert_eq!(sig.algorithm, deserialized.algorithm);
        assert_eq!(sig.public_key, deserialized.public_key);
    }

    #[test]
    fn test_verify_sync_file_success() {
        use ed25519_dalek::{Signer, SigningKey};
        use rand::rngs::OsRng;
        use std::io::Write;

        // Create a temporary .sync file
        let mut temp_file = tempfile::NamedTempFile::new().unwrap();
        let file_content = b"test sync file content";
        temp_file.write_all(file_content).unwrap();
        let file_path = temp_file.path();

        // Generate a key pair
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        // Compute hash
        let hash = blake3::hash(file_content);
        let hash_hex = format!("blake3:{}", hex::encode(hash.as_bytes()));

        // Create did:key
        let mut did_bytes = vec![0xed, 0x01]; // multicodec prefix
        did_bytes.extend_from_slice(&verifying_key.to_bytes());
        let did = format!("did:key:z{}", bs58::encode(&did_bytes).into_string());

        // Sign the hash
        let signature = signing_key.sign(hash.as_bytes());
        let sig_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            signature.to_bytes(),
        );

        // Create signature object
        let sig = SyncSignature {
            algorithm: "Ed25519".to_string(),
            public_key: did,
            signature: sig_b64,
            content_hash: hash_hex,
            signed_at: 1234567890,
        };

        // Verify
        let result = verify_sync_file(file_path, &sig).unwrap();
        assert!(
            result.valid,
            "Verification should succeed: {:?}",
            result.error
        );
        assert!(result.public_key_bytes.is_some());
        assert_eq!(result.computed_hash, sig.content_hash);
    }

    #[test]
    fn test_verify_sync_file_wrong_content() {
        use ed25519_dalek::{Signer, SigningKey};
        use rand::rngs::OsRng;
        use std::io::Write;

        // Create a temporary .sync file
        let mut temp_file = tempfile::NamedTempFile::new().unwrap();
        let file_content = b"test sync file content";
        temp_file.write_all(file_content).unwrap();
        let file_path = temp_file.path();

        // Generate a key pair
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        // Compute hash of DIFFERENT content
        let wrong_content = b"wrong content";
        let hash = blake3::hash(wrong_content);
        let hash_hex = format!("blake3:{}", hex::encode(hash.as_bytes()));

        // Create did:key
        let mut did_bytes = vec![0xed, 0x01];
        did_bytes.extend_from_slice(&verifying_key.to_bytes());
        let did = format!("did:key:z{}", bs58::encode(&did_bytes).into_string());

        // Sign the wrong hash
        let signature = signing_key.sign(hash.as_bytes());
        let sig_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            signature.to_bytes(),
        );

        // Create signature object
        let sig = SyncSignature {
            algorithm: "Ed25519".to_string(),
            public_key: did,
            signature: sig_b64,
            content_hash: hash_hex,
            signed_at: 1234567890,
        };

        // Verify should fail due to hash mismatch
        let result = verify_sync_file(file_path, &sig).unwrap();
        assert!(!result.valid, "Verification should fail");
        assert!(result.error.is_some());
        assert!(result.error.as_ref().unwrap().contains("hash mismatch"));
    }
}
