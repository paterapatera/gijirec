//! Session capture audio controls per `docs/contracts/capture-audio-controls.md`.

use serde::{Deserialize, Serialize};

/// Minimum allowed `manual_ingest_gain` per contract.
pub const MIN_INGEST_GAIN: f32 = 0.25;

/// Maximum allowed `manual_ingest_gain` per contract.
pub const MAX_INGEST_GAIN: f32 = 4.0;

/// Default `manual_ingest_gain` when the user has not adjusted gain in the session.
pub const DEFAULT_INGEST_GAIN: f32 = 1.25;

/// Transcribe ingest mix controls (session-scoped, not persisted to disk).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CaptureAudioControls {
    pub mic_ingest_enabled: bool,
    pub manual_ingest_gain: f32,
    pub gain_user_adjusted: bool,
}

impl Default for CaptureAudioControls {
    fn default() -> Self {
        Self {
            mic_ingest_enabled: true,
            manual_ingest_gain: DEFAULT_INGEST_GAIN,
            gain_user_adjusted: false,
        }
    }
}

/// Errors when validating `manual_ingest_gain`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestGainValidationError {
    NotFinite,
    OutOfRange,
}

/// Validates a manual ingest gain value (finite and within contract bounds).
pub fn validate_manual_ingest_gain(gain: f32) -> Result<f32, IngestGainValidationError> {
    if !gain.is_finite() {
        return Err(IngestGainValidationError::NotFinite);
    }
    if gain < MIN_INGEST_GAIN || gain > MAX_INGEST_GAIN {
        return Err(IngestGainValidationError::OutOfRange);
    }
    Ok(gain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_constraint_constants_match_contract() {
        assert_eq!(MIN_INGEST_GAIN, 0.25);
        assert_eq!(MAX_INGEST_GAIN, 4.0);
        assert_eq!(DEFAULT_INGEST_GAIN, 1.25);
    }

    #[test]
    fn default_controls_match_contract() {
        let controls = CaptureAudioControls::default();
        assert!(controls.mic_ingest_enabled);
        assert_eq!(controls.manual_ingest_gain, DEFAULT_INGEST_GAIN);
        assert!(!controls.gain_user_adjusted);
    }

    #[test]
    fn validate_accepts_boundary_gains() {
        assert_eq!(
            validate_manual_ingest_gain(MIN_INGEST_GAIN).expect("min"),
            MIN_INGEST_GAIN
        );
        assert_eq!(
            validate_manual_ingest_gain(MAX_INGEST_GAIN).expect("max"),
            MAX_INGEST_GAIN
        );
        assert_eq!(
            validate_manual_ingest_gain(DEFAULT_INGEST_GAIN).expect("default"),
            DEFAULT_INGEST_GAIN
        );
    }

    #[test]
    fn validate_rejects_out_of_range_gains() {
        assert_eq!(
            validate_manual_ingest_gain(MIN_INGEST_GAIN - 0.01),
            Err(IngestGainValidationError::OutOfRange)
        );
        assert_eq!(
            validate_manual_ingest_gain(MAX_INGEST_GAIN + 0.01),
            Err(IngestGainValidationError::OutOfRange)
        );
    }

    #[test]
    fn validate_rejects_non_finite_gains() {
        assert_eq!(
            validate_manual_ingest_gain(f32::NAN),
            Err(IngestGainValidationError::NotFinite)
        );
        assert_eq!(
            validate_manual_ingest_gain(f32::INFINITY),
            Err(IngestGainValidationError::NotFinite)
        );
        assert_eq!(
            validate_manual_ingest_gain(f32::NEG_INFINITY),
            Err(IngestGainValidationError::NotFinite)
        );
    }

    #[test]
    fn controls_round_trip_through_serde() {
        let controls = CaptureAudioControls {
            mic_ingest_enabled: false,
            manual_ingest_gain: 2.0,
            gain_user_adjusted: true,
        };
        let json = serde_json::to_string(&controls).expect("serialize");
        let restored: CaptureAudioControls = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(controls, restored);
    }
}
