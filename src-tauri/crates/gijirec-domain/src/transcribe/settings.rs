//! Persisted transcribe preferences (`docs/contracts/whisper-transcribe-settings.md`).

use super::model_variant::WhisperModelVariant;

/// Persisted transcribe settings (`transcribe-settings.json`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TranscribeSettings {
    #[serde(default)]
    pub model_variant: WhisperModelVariant,
}

impl Default for TranscribeSettings {
    fn default() -> Self {
        Self {
            model_variant: WhisperModelVariant::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn default_settings_use_fp16_variant() {
        let settings = TranscribeSettings::default();
        assert_eq!(settings.model_variant, WhisperModelVariant::Fp16);
    }

    #[test]
    fn settings_serializes_contract_shape() {
        let settings = TranscribeSettings::default();
        let value: Value = serde_json::to_value(&settings).expect("serialize");
        let obj = value.as_object().expect("object");
        assert_eq!(
            obj.get("model_variant").and_then(Value::as_str),
            Some("fp16")
        );
        assert_eq!(obj.len(), 1, "settings JSON must contain exactly one field");
    }

    #[test]
    fn settings_round_trips_through_json() {
        let original = TranscribeSettings {
            model_variant: WhisperModelVariant::Q5_0,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: TranscribeSettings = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn deserializes_empty_object_with_fp16_default() {
        let restored: TranscribeSettings =
            serde_json::from_value(json!({})).expect("deserialize empty object");
        assert_eq!(restored, TranscribeSettings::default());
    }
}
