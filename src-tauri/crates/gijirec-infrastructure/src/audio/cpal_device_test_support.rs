//! Shared cpal device lookup and hardware smoke assertions for adapter tests.

use crate::audio::f32_ring_consumer::F32RingConsumer;
#[cfg(test)]
use cpal::traits::DeviceTrait;
#[cfg(test)]
use gijirec_domain::audio::{AudioDeviceId, CaptureError};
#[cfg(test)]
use std::time::Duration;

#[cfg(test)]
pub(crate) const UNKNOWN_DEVICE_ID: &str = "gijirec-nonexistent-device-id-xyz";

#[cfg(test)]
pub(crate) fn unknown_device_id() -> AudioDeviceId {
    AudioDeviceId::new(UNKNOWN_DEVICE_ID.to_string()).expect("valid id")
}

#[cfg(test)]
pub(crate) fn assert_find_device_returns_error_for_unknown_id<F>(find: F, expected: CaptureError)
where
    F: FnOnce(&cpal::Host, &AudioDeviceId) -> Result<cpal::Device, CaptureError>,
{
    let host = cpal::default_host();
    let device_id = unknown_device_id();

    assert!(matches!(find(&host, &device_id), Err(err) if err == expected));
}

#[cfg(test)]
pub(crate) fn assert_resolves_device_when_default_name_matches<F, G>(default_device: F, find: G)
where
    F: FnOnce(&cpal::Host) -> Option<cpal::Device>,
    G: FnOnce(&cpal::Host, &AudioDeviceId) -> Result<cpal::Device, CaptureError>,
{
    let host = cpal::default_host();
    let default_device = match default_device(&host) {
        Some(device) => device,
        None => return,
    };
    let name = match default_device.name() {
        Ok(name) => name,
        Err(_) => return,
    };
    let device_id = AudioDeviceId::new(name).expect("valid id");

    let found = find(&host, &device_id).expect("device found");
    assert_eq!(found.name().expect("name"), device_id.as_str());
}

#[cfg(test)]
pub(crate) fn assert_cpal_consumer_receives_samples(
    sample_rate_hz: u32,
    mut consumer: F32RingConsumer,
    sleep_ms: u64,
) {
    assert!(sample_rate_hz > 0);
    std::thread::sleep(Duration::from_millis(sleep_ms));
    assert!(consumer.slots() > 0 || consumer.pop().is_some());
}
