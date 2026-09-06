//! Editor settings persisted as JSON (`docs/contracts/transcript-editor-settings.md`).

/// Persisted editor preferences (`save_directory`, `export_jsonl_enabled`).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EditorSettings {
    #[serde(default)]
    pub save_directory: Option<String>,
    #[serde(default)]
    pub export_jsonl_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::EditorSettings;
    use serde_json::{Value, json};

    #[test]
    fn default_settings_serializes_contract_shape() {
        let settings = EditorSettings::default();
        let value: Value = serde_json::to_value(&settings).expect("serialize default settings");
        let obj = value.as_object().expect("object");
        assert_eq!(obj.get("save_directory"), Some(&Value::Null));
        assert_eq!(
            obj.get("export_jsonl_enabled").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            obj.len(),
            2,
            "settings JSON must contain exactly two fields"
        );
    }

    #[test]
    fn settings_round_trips_through_json() {
        let original = EditorSettings {
            save_directory: Some(r"C:\Users\me\gijirec".to_string()),
            export_jsonl_enabled: true,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: EditorSettings = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }

    #[test]
    fn deserializes_partial_json_with_defaults() {
        let restored: EditorSettings =
            serde_json::from_value(json!({ "save_directory": null })).expect("deserialize");
        assert_eq!(restored, EditorSettings::default());
    }
}
