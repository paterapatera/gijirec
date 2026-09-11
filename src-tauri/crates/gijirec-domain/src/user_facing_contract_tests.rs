//! Shared assertions for user-facing error contract tests.

use serde::Serialize;
use serde::de::DeserializeOwned;

pub(crate) trait UserFacingContractPayload {
    fn contract_code_str(&self) -> &str;
    fn message_ja(&self) -> &str;
    fn action_ja_is_present(&self) -> bool;
}

pub(crate) fn assert_contract_error_mappings<E, U, F>(
    errors: Vec<E>,
    expected_codes: &[&str],
    to_user_facing: F,
) where
    F: Fn(&E) -> U,
    U: UserFacingContractPayload,
{
    assert_eq!(
        errors.len(),
        expected_codes.len(),
        "every contract error code must have a mapping"
    );

    for (error, code) in errors.iter().zip(expected_codes.iter()) {
        let facing = to_user_facing(error);
        assert_eq!(
            facing.contract_code_str(),
            *code,
            "unexpected contract code mapping"
        );
        assert!(
            facing.action_ja_is_present(),
            "action_ja must be non-empty for contract code {code}"
        );
        assert!(
            !facing.message_ja().trim().is_empty(),
            "message_ja must be non-empty for contract code {code}"
        );
    }
}

fn assert_json_contract_shape(json: &serde_json::Value, expected_code: &str, field_count: usize) {
    let obj = json.as_object().expect("object payload");
    assert_eq!(
        obj.get("code").and_then(|v| v.as_str()),
        Some(expected_code)
    );
    assert!(obj.get("message_ja").and_then(|v| v.as_str()).is_some());
    assert!(obj.get("action_ja").and_then(|v| v.as_str()).is_some());
    if field_count == 4 {
        assert_eq!(obj.get("recoverable").and_then(|v| v.as_bool()), Some(true));
    }
    assert_eq!(obj.len(), field_count);
}

pub fn assert_invoke_error_serializes_contract_shape<T: Serialize>(err: &T, expected_code: &str) {
    let json = serde_json::to_value(err).expect("serialize");
    assert_json_contract_shape(&json, expected_code, 3);
}

pub(crate) fn assert_serde_produces_contract_json_shape<T: Serialize>(
    facing: &T,
    expected_code: &str,
) {
    let json = serde_json::to_value(facing).expect("serialize user-facing error");
    assert_json_contract_shape(&json, expected_code, 4);
}

pub(crate) fn assert_all_errors_serde_round_trip<E, U, F>(errors: Vec<E>, to_user_facing: F)
where
    F: Fn(&E) -> U,
    U: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    for error in &errors {
        let facing = to_user_facing(error);
        assert_serde_round_trips(&facing);
    }
}

pub(crate) fn assert_serde_round_trips<T>(facing: &T)
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_string(facing).expect("serialize");
    let restored: T = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(*facing, restored);
}

macro_rules! impl_user_facing_contract_payload {
    ($ty:ty) => {
        impl UserFacingContractPayload for $ty {
            fn contract_code_str(&self) -> &str {
                self.code.as_str()
            }

            fn message_ja(&self) -> &str {
                &self.message_ja
            }

            fn action_ja_is_present(&self) -> bool {
                self.action_ja_is_present()
            }
        }
    };
}

impl_user_facing_contract_payload!(crate::audio::error::UserFacingError);
impl_user_facing_contract_payload!(crate::transcribe::error::UserFacingTranscribeError);
impl_user_facing_contract_payload!(crate::editor::error::EditorUserError);
