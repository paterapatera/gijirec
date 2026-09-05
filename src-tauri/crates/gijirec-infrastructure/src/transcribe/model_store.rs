//! Local whisper model path resolution and integrity verification.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use gijirec_domain::transcribe::TranscribeError;
use sha2::{Digest, Sha256};

/// Default whisper model filename stored under `{app_data_dir}/models/`.
pub const MODEL_FILENAME: &str = "kotoba-whisper-v2.2-ggml-q5_0.bin";

const MODELS_SUBDIR: &str = "models";

/// Resolves and validates the local whisper model under Tauri `app_data_dir`.
pub struct ModelStore {
    base_data_dir: PathBuf,
}

impl ModelStore {
    pub fn new(base_data_dir: PathBuf) -> Self {
        Self { base_data_dir }
    }

    pub fn models_dir(&self) -> PathBuf {
        self.base_data_dir.join(MODELS_SUBDIR)
    }

    pub fn model_path(&self) -> PathBuf {
        self.models_dir().join(MODEL_FILENAME)
    }

    pub fn verify(&self, expected_sha256: Option<&str>) -> Result<PathBuf, TranscribeError> {
        let path = self.model_path();
        if !path.exists() {
            return Err(TranscribeError::ModelNotFound {
                detail: format!("model file not found at {}", path.display()),
            });
        }

        match compute_sha256_hex(&path) {
            Ok(actual) => {
                if let Some(expected) = expected_sha256
                    && !sha256_matches(expected, &actual)
                {
                    remove_corrupt_file(&path);
                    return Err(TranscribeError::ModelCorrupt {
                        detail: format!(
                            "model checksum mismatch at {}: expected {expected}, got {actual}",
                            path.display()
                        ),
                    });
                }
                Ok(path)
            }
            Err(err) => {
                remove_corrupt_file(&path);
                Err(TranscribeError::ModelCorrupt {
                    detail: format!("failed to read model file at {}: {err}", path.display()),
                })
            }
        }
    }

    pub fn delete_model(&self) -> Result<(), TranscribeError> {
        let path = self.model_path();
        if !path.exists() {
            return Ok(());
        }

        fs::remove_file(&path).map_err(|err| TranscribeError::Internal {
            detail: format!("failed to delete model at {}: {err}", path.display()),
        })
    }
}

fn compute_sha256_hex(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn sha256_matches(expected: &str, actual: &str) -> bool {
    expected.eq_ignore_ascii_case(actual)
}

fn remove_corrupt_file(path: &Path) {
    let _ = fs::remove_file(path);
    if path.exists() {
        let _ = fs::remove_dir_all(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gijirec_domain::transcribe::TranscribeErrorCode;
    use sha2::{Digest, Sha256};

    fn temp_store() -> (ModelStore, PathBuf) {
        let base = std::env::temp_dir().join(format!(
            "gijirec-model-store-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp base dir");
        (ModelStore::new(base.clone()), base)
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn cleanup(base: &Path) {
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn model_path_resolves_under_models_dir() {
        let (store, base) = temp_store();
        assert_eq!(store.models_dir(), base.join("models"));
        assert_eq!(store.model_path(), base.join("models").join(MODEL_FILENAME));
        cleanup(&base);
    }

    #[test]
    fn verify_returns_model_not_found_when_file_missing() {
        let (store, base) = temp_store();
        let err = store.verify(None).expect_err("missing model should fail");

        assert!(matches!(err, TranscribeError::ModelNotFound { .. }));
        assert_eq!(
            err.to_user_facing().code,
            TranscribeErrorCode::ModelNotFound
        );
        cleanup(&base);
    }

    #[test]
    fn verify_returns_ok_for_valid_file_with_matching_checksum() {
        let (store, base) = temp_store();
        let models_dir = store.models_dir();
        fs::create_dir_all(&models_dir).expect("create models dir");
        let content = b"valid-local-whisper-model-bytes";
        fs::write(store.model_path(), content).expect("write model file");

        let expected = sha256_hex(content);
        let path = store
            .verify(Some(&expected))
            .expect("valid model should verify");
        assert_eq!(path, store.model_path());
        assert!(path.exists());

        cleanup(&base);
    }

    #[test]
    fn verify_removes_corrupt_file_on_checksum_mismatch() {
        let (store, base) = temp_store();
        let models_dir = store.models_dir();
        fs::create_dir_all(&models_dir).expect("create models dir");
        fs::write(store.model_path(), b"partial-or-corrupt-download")
            .expect("write corrupt model file");

        let err = store
            .verify(Some("deadbeef"))
            .expect_err("checksum mismatch should fail");

        assert!(matches!(err, TranscribeError::ModelCorrupt { .. }));
        assert_eq!(err.to_user_facing().code, TranscribeErrorCode::ModelCorrupt);
        assert!(
            !store.model_path().exists(),
            "corrupt model file must be removed"
        );

        cleanup(&base);
    }

    #[test]
    fn verify_removes_unreadable_model_file() {
        let (store, base) = temp_store();
        let models_dir = store.models_dir();
        fs::create_dir_all(&models_dir).expect("create models dir");
        fs::create_dir(store.model_path()).expect("occupy model path with directory");

        let err = store
            .verify(None)
            .expect_err("unreadable model should fail");
        assert!(matches!(err, TranscribeError::ModelCorrupt { .. }));
        assert!(
            !store.model_path().exists(),
            "unreadable model path must be removed when possible"
        );

        cleanup(&base);
    }

    #[test]
    fn delete_model_removes_existing_file() {
        let (store, base) = temp_store();
        let models_dir = store.models_dir();
        fs::create_dir_all(&models_dir).expect("create models dir");
        fs::write(store.model_path(), b"delete-me").expect("write model file");

        store.delete_model().expect("delete existing model");
        assert!(!store.model_path().exists());

        cleanup(&base);
    }
}
