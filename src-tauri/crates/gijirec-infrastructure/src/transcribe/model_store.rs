//! Local whisper model path resolution and integrity verification.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use gijirec_domain::transcribe::{ModelVariantCatalog, TranscribeError, WhisperModelVariant};
use sha2::{Digest, Sha256};

/// Default FP16 whisper model filename stored under `{app_data_dir}/models/`.
pub const MODEL_FILENAME: &str = "kotoba-whisper-v2.2-ggml.bin";

const MODELS_SUBDIR: &str = "models";

/// Resolves and validates local whisper models under Tauri `app_data_dir`.
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

    /// FP16 model path (既存 API 後方互換).
    pub fn model_path(&self) -> PathBuf {
        self.model_path_for(WhisperModelVariant::Fp16)
    }

    /// Resolves the on-disk path for a variant.
    pub fn model_path_for(&self, variant: WhisperModelVariant) -> PathBuf {
        let filename = ModelVariantCatalog::get(variant).filename;
        self.models_dir().join(filename)
    }

    /// Returns whether the variant's model file exists (existence only, no verification).
    pub fn file_exists(&self, variant: WhisperModelVariant) -> bool {
        self.model_path_for(variant).is_file()
    }

    /// Verifies the FP16 model (既存 API 後方互換).
    pub fn verify(&self, expected_sha256: Option<&str>) -> Result<PathBuf, TranscribeError> {
        self.verify_variant(WhisperModelVariant::Fp16, expected_sha256)
    }

    /// Verifies a variant's local model file and checksum.
    pub fn verify_variant(
        &self,
        variant: WhisperModelVariant,
        expected_sha256: Option<&str>,
    ) -> Result<PathBuf, TranscribeError> {
        let path = self.model_path_for(variant);
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
        self.delete_variant(WhisperModelVariant::Fp16)
    }

    pub fn delete_variant(&self, variant: WhisperModelVariant) -> Result<(), TranscribeError> {
        let path = self.model_path_for(variant);
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
        let base = crate::transcribe::test_temp::unique_temp_path("gijirec-model-store");
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

    #[test]
    fn model_path_for_resolves_all_three_variants() {
        use gijirec_domain::transcribe::{ModelVariantCatalog, WhisperModelVariant};

        let (store, base) = temp_store();
        for descriptor in ModelVariantCatalog::all() {
            let path = store.model_path_for(descriptor.variant);
            assert_eq!(
                path,
                base.join("models").join(descriptor.filename),
                "path for {:?}",
                descriptor.variant
            );
        }
        assert_eq!(
            store.model_path(),
            store.model_path_for(WhisperModelVariant::Fp16)
        );
        cleanup(&base);
    }

    #[test]
    fn verify_variant_validates_per_variant_file() {
        use gijirec_domain::transcribe::WhisperModelVariant;

        let (store, base) = temp_store();
        fs::create_dir_all(store.models_dir()).expect("create models dir");
        let content = b"q5-model-bytes";
        fs::write(store.model_path_for(WhisperModelVariant::Q5_0), content)
            .expect("write q5 model");

        let expected = sha256_hex(content);
        let path = store
            .verify_variant(WhisperModelVariant::Q5_0, Some(&expected))
            .expect("verify q5 model");
        assert_eq!(path, store.model_path_for(WhisperModelVariant::Q5_0));

        cleanup(&base);
    }

    #[test]
    fn existing_fp16_file_verifies_without_rename() {
        use gijirec_domain::transcribe::{ModelVariantCatalog, WhisperModelVariant};

        let (store, base) = temp_store();
        fs::create_dir_all(store.models_dir()).expect("create models dir");
        let content = b"existing-fp16-model";
        fs::write(store.model_path(), content).expect("write fp16 model");

        let expected = sha256_hex(content);
        let path = store
            .verify_variant(WhisperModelVariant::Fp16, Some(&expected))
            .expect("fp16 verify");
        assert_eq!(
            path.file_name().map(|n| n.to_string_lossy()),
            Some(ModelVariantCatalog::fp16().filename.into())
        );
        assert!(store.file_exists(WhisperModelVariant::Fp16));
        assert!(!store.file_exists(WhisperModelVariant::Q5_0));

        cleanup(&base);
    }
}
