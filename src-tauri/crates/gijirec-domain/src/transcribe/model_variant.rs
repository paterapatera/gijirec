//! kotoba-whisper-v2.2 quantization variants and catalog metadata.
//!
//! Authoritative contract: `docs/contracts/whisper-transcribe-settings.md`

use serde::{Deserialize, Serialize};

/// kotoba-whisper-v2.2 の 3 バリアントのみ（要件 1.3–1.4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WhisperModelVariant {
    Q5_0,
    Q8_0,
    #[default]
    Fp16,
}

/// Single catalog row: filename, distribution URL, and SHA-256 for verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelVariantDescriptor {
    pub variant: WhisperModelVariant,
    pub filename: &'static str,
    pub url: &'static str,
    pub expected_sha256: &'static str,
}

/// Canonical metadata for all supported whisper model variants.
pub struct ModelVariantCatalog;

impl ModelVariantCatalog {
    const Q5_0: ModelVariantDescriptor = ModelVariantDescriptor {
        variant: WhisperModelVariant::Q5_0,
        filename: "kotoba-whisper-v2.2-ggml-q5_0.bin",
        url: "https://huggingface.co/kenrouse/kotoba-whisper-v2.2-ggml/resolve/main/kotoba-whisper-v2.2-ggml-q5_0.bin",
        expected_sha256: "4a3b92192b5d3578ff854a5876213e2e27af0c2d357492c2d14271e82c303658",
    };

    const Q8_0: ModelVariantDescriptor = ModelVariantDescriptor {
        variant: WhisperModelVariant::Q8_0,
        filename: "kotoba-whisper-v2.2-ggml-q8_0.bin",
        url: "https://huggingface.co/kenrouse/kotoba-whisper-v2.2-ggml/resolve/main/kotoba-whisper-v2.2-ggml-q8_0.bin",
        expected_sha256: "c4071b2f8f0129d463c6c7fd2e72c82f7276f9882a8f9cd9474e0c2b699100c4",
    };

    const FP16: ModelVariantDescriptor = ModelVariantDescriptor {
        variant: WhisperModelVariant::Fp16,
        filename: "kotoba-whisper-v2.2-ggml.bin",
        url: "https://huggingface.co/kenrouse/kotoba-whisper-v2.2-ggml/resolve/main/kotoba-whisper-v2.2-ggml.bin",
        expected_sha256: "eff70a8a236e731abba774ba71e1f6d0fce53302137208c32207e694e0bf4546",
    };

    /// All three supported variants in stable order (Q5_0, Q8_0, FP16).
    pub fn all() -> &'static [ModelVariantDescriptor] {
        &[Self::Q5_0, Self::Q8_0, Self::FP16]
    }

    /// Lookup descriptor for a variant.
    pub fn get(variant: WhisperModelVariant) -> &'static ModelVariantDescriptor {
        match variant {
            WhisperModelVariant::Q5_0 => &Self::Q5_0,
            WhisperModelVariant::Q8_0 => &Self::Q8_0,
            WhisperModelVariant::Fp16 => &Self::FP16,
        }
    }

    /// FP16 descriptor (既存利用者後方互換の既定バリアント).
    pub fn fp16() -> &'static ModelVariantDescriptor {
        &Self::FP16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    /// Contract table from `docs/contracts/whisper-transcribe-settings.md`.
    const CONTRACT_ROWS: [(&str, &str, &str, &str); 3] = [
        (
            "q5_0",
            "kotoba-whisper-v2.2-ggml-q5_0.bin",
            "https://huggingface.co/kenrouse/kotoba-whisper-v2.2-ggml/resolve/main/kotoba-whisper-v2.2-ggml-q5_0.bin",
            "4a3b92192b5d3578ff854a5876213e2e27af0c2d357492c2d14271e82c303658",
        ),
        (
            "q8_0",
            "kotoba-whisper-v2.2-ggml-q8_0.bin",
            "https://huggingface.co/kenrouse/kotoba-whisper-v2.2-ggml/resolve/main/kotoba-whisper-v2.2-ggml-q8_0.bin",
            "c4071b2f8f0129d463c6c7fd2e72c82f7276f9882a8f9cd9474e0c2b699100c4",
        ),
        (
            "fp16",
            "kotoba-whisper-v2.2-ggml.bin",
            "https://huggingface.co/kenrouse/kotoba-whisper-v2.2-ggml/resolve/main/kotoba-whisper-v2.2-ggml.bin",
            "eff70a8a236e731abba774ba71e1f6d0fce53302137208c32207e694e0bf4546",
        ),
    ];

    #[test]
    fn default_variant_is_fp16() {
        assert_eq!(WhisperModelVariant::default(), WhisperModelVariant::Fp16);
    }

    #[test]
    fn serde_uses_contract_snake_case_values() {
        let cases = [
            (WhisperModelVariant::Q5_0, "q5_0"),
            (WhisperModelVariant::Q8_0, "q8_0"),
            (WhisperModelVariant::Fp16, "fp16"),
        ];
        for (variant, expected) in cases {
            let value = serde_json::to_value(variant).expect("serialize");
            assert_eq!(value, Value::String(expected.to_string()));
            let restored: WhisperModelVariant =
                serde_json::from_value(Value::String(expected.to_string())).expect("deserialize");
            assert_eq!(restored, variant);
        }
    }

    #[test]
    fn rejects_unknown_variant_values() {
        let err = serde_json::from_value::<WhisperModelVariant>(json!("q4_0"))
            .expect_err("unknown variant must fail");
        assert!(err.to_string().contains("unknown"), "{}", err);
    }

    #[test]
    fn catalog_has_exactly_three_entries() {
        assert_eq!(ModelVariantCatalog::all().len(), 3);
    }

    #[test]
    fn catalog_metadata_matches_contract_byte_for_byte() {
        let catalog = ModelVariantCatalog::all();
        assert_eq!(catalog.len(), CONTRACT_ROWS.len());

        for (descriptor, (serde_key, filename, url, sha)) in
            catalog.iter().zip(CONTRACT_ROWS.iter())
        {
            let serialized = serde_json::to_value(descriptor.variant).expect("serialize variant");
            assert_eq!(serialized, Value::String(serde_key.to_string()));
            assert_eq!(descriptor.filename, *filename);
            assert_eq!(descriptor.url, *url);
            assert_eq!(descriptor.expected_sha256, *sha);
        }
    }

    #[test]
    fn get_returns_same_descriptor_as_all_entries() {
        for descriptor in ModelVariantCatalog::all() {
            assert_eq!(
                ModelVariantCatalog::get(descriptor.variant),
                descriptor,
                "get({:?})",
                descriptor.variant
            );
        }
    }

    #[test]
    fn fp16_returns_fp16_descriptor() {
        assert_eq!(
            ModelVariantCatalog::fp16().variant,
            WhisperModelVariant::Fp16
        );
        assert_eq!(
            ModelVariantCatalog::fp16().filename,
            "kotoba-whisper-v2.2-ggml.bin"
        );
    }
}
