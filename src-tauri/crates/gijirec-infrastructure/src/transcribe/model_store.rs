//! Local whisper model path resolution and integrity verification.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use gijirec_domain::transcribe::TranscribeError;
use sha2::{Digest, Sha256};

/// Default whisper model filename stored under `{app_data_dir}/models/`.
pub const MODEL_FILENAME: &str = "kotoba-whisper-v2.2-ggml.bin";

const MODELS_SUBDIR: &str = "models";
const LEGACY_APP_SUBDIR: &str = "gijirec";

/// Resolves and validates the local whisper model under Tauri `app_data_dir`.
pub struct ModelStore {
    base_data_dir: PathBuf,
}

impl ModelStore {
    pub fn new(base_data_dir: PathBuf) -> Self {
        Self { base_data_dir }
    }

    /// Legacy `%LOCALAPPDATA%/gijirec/models/` directory (pre ADR-0008).
    pub fn legacy_local_models_dir() -> Option<PathBuf> {
        dirs::data_local_dir().map(|local| local.join(LEGACY_APP_SUBDIR).join(MODELS_SUBDIR))
    }

    /// Copies a legacy local model into `{app_data_dir}/models/` when present.
    /// Copy failures are ignored so the existing download flow can recover.
    pub fn maybe_migrate_from_legacy_local(&self) -> Result<(), TranscribeError> {
        if self.base_data_dir.as_os_str().is_empty() {
            return Ok(());
        }
        let Some(legacy_models_dir) = Self::legacy_local_models_dir() else {
            return Ok(());
        };
        self.maybe_migrate_from_legacy_models_dir(&legacy_models_dir)
    }

    /// Copies from an explicit legacy models directory (tests / overrides).
    pub fn maybe_migrate_from_legacy_models_dir(
        &self,
        legacy_models_dir: &Path,
    ) -> Result<(), TranscribeError> {
        if self.base_data_dir.as_os_str().is_empty() {
            return Ok(());
        }
        let destination = self.model_path();
        if destination.exists() {
            return Ok(());
        }

        let legacy_path = legacy_models_dir.join(MODEL_FILENAME);
        if !legacy_path.is_file() {
            return Ok(());
        }

        if let Some(parent) = destination.parent()
            && fs::create_dir_all(parent).is_err()
        {
            return Ok(());
        }

        match fs::copy(&legacy_path, &destination) {
            Ok(_) => Ok(()),
            Err(_) => {
                remove_corrupt_file(&destination);
                Ok(())
            }
        }
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

    fn temp_legacy_models_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "gijirec-legacy-models-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    #[test]
    fn maybe_migrate_from_legacy_models_dir_skips_empty_base_data_dir() {
        let legacy_models = temp_legacy_models_dir();
        fs::create_dir_all(&legacy_models).expect("create legacy models dir");
        fs::write(legacy_models.join(MODEL_FILENAME), b"legacy-copy-me")
            .expect("write legacy model");

        let store = ModelStore::new(PathBuf::new());
        let cwd_pollution = std::env::current_dir()
            .expect("cwd")
            .join("models")
            .join(MODEL_FILENAME);

        store
            .maybe_migrate_from_legacy_models_dir(&legacy_models)
            .expect("empty base must skip migration");

        assert!(
            !cwd_pollution.exists(),
            "empty base_data_dir must not copy into cwd/models"
        );

        cleanup(&legacy_models);
    }

    #[test]
    fn maybe_migrate_from_legacy_models_dir_copies_model_for_verify() {
        let (store, base) = temp_store();
        let legacy_models = temp_legacy_models_dir();
        fs::create_dir_all(&legacy_models).expect("create legacy models dir");
        let content = b"migrated-whisper-model-bytes";
        fs::write(legacy_models.join(MODEL_FILENAME), content).expect("write legacy model");

        store
            .maybe_migrate_from_legacy_models_dir(&legacy_models)
            .expect("migration should succeed");

        let expected = sha256_hex(content);
        store
            .verify(Some(&expected))
            .expect("migrated model should verify");

        cleanup(&base);
        cleanup(&legacy_models);
    }

    #[test]
    fn maybe_migrate_from_legacy_models_dir_skips_when_destination_exists() {
        let (store, base) = temp_store();
        let legacy_models = temp_legacy_models_dir();
        fs::create_dir_all(&legacy_models).expect("create legacy models dir");
        fs::write(legacy_models.join(MODEL_FILENAME), b"legacy-only").expect("write legacy model");

        fs::create_dir_all(store.models_dir()).expect("create destination models dir");
        fs::write(store.model_path(), b"already-here").expect("write destination model");

        store
            .maybe_migrate_from_legacy_models_dir(&legacy_models)
            .expect("migration should no-op");

        let contents = fs::read(store.model_path()).expect("read destination model");
        assert_eq!(contents, b"already-here");

        cleanup(&base);
        cleanup(&legacy_models);
    }

    #[test]
    fn maybe_migrate_from_legacy_models_dir_skips_when_legacy_missing() {
        let (store, base) = temp_store();
        let legacy_models = temp_legacy_models_dir();

        store
            .maybe_migrate_from_legacy_models_dir(&legacy_models)
            .expect("missing legacy should no-op");

        assert!(matches!(
            store.verify(None),
            Err(TranscribeError::ModelNotFound { .. })
        ));

        cleanup(&base);
    }

    #[test]
    fn maybe_migrate_from_legacy_models_dir_failure_leaves_model_not_found_for_download() {
        let (store, base) = temp_store();
        let legacy_models = temp_legacy_models_dir();
        fs::create_dir_all(&legacy_models).expect("create legacy models dir");
        fs::write(legacy_models.join(MODEL_FILENAME), b"legacy-copy-me")
            .expect("write legacy model");
        fs::write(store.models_dir(), b"not-a-directory").expect("block models dir path");

        store
            .maybe_migrate_from_legacy_models_dir(&legacy_models)
            .expect("copy failure should delegate to download flow");

        assert!(matches!(
            store.verify(None),
            Err(TranscribeError::ModelNotFound { .. })
        ));

        cleanup(&base);
        cleanup(&legacy_models);
    }
}
