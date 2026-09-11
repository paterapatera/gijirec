//! Shared audio device fixtures for unit tests across crates.

use super::pcm_chunk::{CHUNK_FRAME_COUNT, PcmChunk};
use super::{AudioDeviceId, AudioDeviceInfo, AudioDeviceKind, AudioDeviceList, DeviceSelection};

pub fn sample_pcm_chunk(sequence: u64) -> PcmChunk {
    PcmChunk::new(
        sequence,
        vec![0_i16; CHUNK_FRAME_COUNT as usize],
        sequence * 100,
    )
    .expect("valid chunk")
}

pub fn assert_both_channels_resolve_to_os_default(selection: &DeviceSelection) {
    assert!(selection.microphone_id().is_none());
    assert!(selection.speaker_id().is_none());
    assert!(selection.resolves_microphone_to_os_default());
    assert!(selection.resolves_speaker_to_os_default());
}

#[allow(dead_code)]
pub fn mic(id: &str, default: bool) -> AudioDeviceInfo {
    AudioDeviceInfo::new(
        AudioDeviceId::new(id.to_string()).expect("id"),
        format!("Mic {id}"),
        AudioDeviceKind::Input,
        default,
    )
}

#[allow(dead_code)]
pub fn speaker(id: &str, default: bool) -> AudioDeviceInfo {
    AudioDeviceInfo::new(
        AudioDeviceId::new(id.to_string()).expect("id"),
        format!("Speaker {id}"),
        AudioDeviceKind::Output,
        default,
    )
}

#[allow(dead_code)]
pub fn sample_device_list() -> AudioDeviceList {
    AudioDeviceList {
        inputs: vec![mic("mic-default", true), mic("mic-usb", false)],
        outputs: vec![speaker("spk-default", true), speaker("spk-hdmi", false)],
    }
}
