//! Mic and system audio infrastructure adapters as orchestrator ports.

use gijirec_presentation::application::capture::orchestrator::{
    MicCapturePort, SystemAudioCapturePort,
};
use gijirec_presentation::domain::audio::CaptureError;
use gijirec_presentation::infrastructure::audio::mic_capture::{
    DEFAULT_RING_CAPACITY, MicCaptureAdapter, MicSampleConsumer,
};
use gijirec_presentation::tauri::observability;
use std::sync::{Arc, Mutex};

#[cfg(target_os = "windows")]
use gijirec_presentation::infrastructure::audio::LoopbackSampleConsumer;
#[cfg(target_os = "macos")]
use gijirec_presentation::infrastructure::audio::SckAudioSampleConsumer;

/// Platform-specific system audio consumer held for the processing thread.
pub(crate) enum SystemSampleConsumer {
    #[cfg(target_os = "windows")]
    Loopback(LoopbackSampleConsumer),
    #[cfg(target_os = "macos")]
    Sck(SckAudioSampleConsumer),
    #[cfg(target_os = "linux")]
    Unavailable,
}

pub(crate) struct MicStreamShared {
    consumer: Mutex<Option<MicSampleConsumer>>,
    sample_rate_hz: Mutex<Option<u32>>,
}

pub(crate) struct SystemStreamShared {
    consumer: Mutex<Option<SystemSampleConsumer>>,
    sample_rate_hz: Mutex<Option<u32>>,
}

/// Shared handles for handing rtrb consumers to the processing thread after open.
#[derive(Clone)]
pub(crate) struct CaptureStreamHandles {
    mic: Arc<MicStreamShared>,
    system: Arc<SystemStreamShared>,
}

impl CaptureStreamHandles {
    pub(crate) fn new_pair() -> (Self, MicPortAdapter, SystemPortAdapter) {
        let mic = Arc::new(MicStreamShared {
            consumer: Mutex::new(None),
            sample_rate_hz: Mutex::new(None),
        });
        let system = Arc::new(SystemStreamShared {
            consumer: Mutex::new(None),
            sample_rate_hz: Mutex::new(None),
        });
        let handles = Self {
            mic: Arc::clone(&mic),
            system: Arc::clone(&system),
        };
        (
            handles,
            MicPortAdapter::new(mic),
            SystemPortAdapter::new(system),
        )
    }

    pub(crate) fn take_mic_consumer(&self) -> Option<MicSampleConsumer> {
        self.mic.consumer.lock().expect("lock").take()
    }

    pub(crate) fn take_system_consumer(&self) -> Option<SystemSampleConsumer> {
        self.system.consumer.lock().expect("lock").take()
    }

    pub(crate) fn mic_sample_rate_hz(&self) -> Option<u32> {
        *self.mic.sample_rate_hz.lock().expect("lock")
    }

    pub(crate) fn system_sample_rate_hz(&self) -> Option<u32> {
        *self.system.sample_rate_hz.lock().expect("lock")
    }

    /// Installs synthetic rtrb consumers for integration tests (no hardware).
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn install_synthetic_mic(&self, sample_rate_hz: u32) {
        let (_prod, cons) = rtrb::RingBuffer::<f32>::new(DEFAULT_RING_CAPACITY);
        *self.mic.consumer.lock().expect("lock") =
            Some(MicSampleConsumer::from_ring_consumer(cons));
        *self.mic.sample_rate_hz.lock().expect("lock") = Some(sample_rate_hz);
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn clear_mic(&self) {
        *self.mic.consumer.lock().expect("lock") = None;
        *self.mic.sample_rate_hz.lock().expect("lock") = None;
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn install_synthetic_system(&self, sample_rate_hz: u32) {
        let (_prod, cons) = rtrb::RingBuffer::<f32>::new(DEFAULT_RING_CAPACITY);
        install_system_consumer(&self.system, cons);
        *self.system.sample_rate_hz.lock().expect("lock") = Some(sample_rate_hz);
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn clear_system(&self) {
        *self.system.consumer.lock().expect("lock") = None;
        *self.system.sample_rate_hz.lock().expect("lock") = None;
    }
}

#[cfg(test)]
#[allow(dead_code)]
fn install_system_consumer(shared: &SystemStreamShared, cons: rtrb::Consumer<f32>) {
    #[cfg(target_os = "windows")]
    {
        *shared.consumer.lock().expect("lock") = Some(SystemSampleConsumer::Loopback(
            LoopbackSampleConsumer::from_ring_consumer(cons),
        ));
    }
    #[cfg(target_os = "macos")]
    {
        *shared.consumer.lock().expect("lock") = Some(SystemSampleConsumer::Sck(
            SckAudioSampleConsumer::from_ring_consumer(cons),
        ));
    }
    #[cfg(target_os = "linux")]
    {
        let _ = (shared, cons);
    }
}

/// Mic port that installs synthetic streams into shared handles (integration tests).
#[cfg(test)]
#[allow(dead_code)]
pub(crate) struct SyntheticMicPort {
    handles: CaptureStreamHandles,
    opened: Arc<Mutex<bool>>,
}

#[cfg(test)]
#[allow(dead_code)]
impl SyntheticMicPort {
    pub(crate) fn new(handles: CaptureStreamHandles, opened: Arc<Mutex<bool>>) -> Self {
        Self { handles, opened }
    }
}

#[cfg(test)]
impl MicCapturePort for SyntheticMicPort {
    fn open(&mut self) -> Result<(), CaptureError> {
        use gijirec_presentation::domain::audio::pcm_chunk::SAMPLE_RATE_HZ;
        self.handles.install_synthetic_mic(SAMPLE_RATE_HZ);
        *self.opened.lock().expect("lock") = true;
        Ok(())
    }

    fn close(&mut self) {
        self.handles.clear_mic();
        *self.opened.lock().expect("lock") = false;
    }

    fn is_open(&self) -> bool {
        *self.opened.lock().expect("lock")
    }
}

/// System port that installs synthetic streams into shared handles (integration tests).
#[cfg(test)]
#[allow(dead_code)]
pub(crate) struct SyntheticSystemPort {
    handles: CaptureStreamHandles,
    opened: Arc<Mutex<bool>>,
}

#[cfg(test)]
#[allow(dead_code)]
impl SyntheticSystemPort {
    pub(crate) fn new(handles: CaptureStreamHandles, opened: Arc<Mutex<bool>>) -> Self {
        Self { handles, opened }
    }
}

#[cfg(test)]
impl SystemAudioCapturePort for SyntheticSystemPort {
    fn open(&mut self) -> Result<(), CaptureError> {
        use gijirec_presentation::domain::audio::pcm_chunk::SAMPLE_RATE_HZ;
        self.handles.install_synthetic_system(SAMPLE_RATE_HZ);
        *self.opened.lock().expect("lock") = true;
        Ok(())
    }

    fn close(&mut self) {
        self.handles.clear_system();
        *self.opened.lock().expect("lock") = false;
    }

    fn is_open(&self) -> bool {
        *self.opened.lock().expect("lock")
    }
}

/// [`MicCapturePort`] backed by [`MicCaptureAdapter`].
pub(crate) struct MicPortAdapter {
    adapter: Option<MicCaptureAdapter>,
    shared: Arc<MicStreamShared>,
}

impl MicPortAdapter {
    pub(crate) fn new(shared: Arc<MicStreamShared>) -> Self {
        Self {
            adapter: None,
            shared,
        }
    }
}

impl Default for MicPortAdapter {
    fn default() -> Self {
        let (handles, mic, _) = CaptureStreamHandles::new_pair();
        let _ = handles;
        mic
    }
}

impl MicCapturePort for MicPortAdapter {
    fn open(&mut self) -> Result<(), CaptureError> {
        if self.adapter.is_some() {
            return Ok(());
        }
        match MicCaptureAdapter::open(DEFAULT_RING_CAPACITY) {
            Ok((adapter, consumer, sample_rate_hz)) => {
                *self.shared.consumer.lock().expect("lock") = Some(consumer);
                *self.shared.sample_rate_hz.lock().expect("lock") = Some(sample_rate_hz);
                self.adapter = Some(adapter);
                Ok(())
            }
            Err(err) => {
                observability::log_stream_open_failure("mic", &err, observability::session_id());
                Err(err)
            }
        }
    }

    fn close(&mut self) {
        *self.shared.consumer.lock().expect("lock") = None;
        *self.shared.sample_rate_hz.lock().expect("lock") = None;
        self.adapter = None;
    }

    fn is_open(&self) -> bool {
        self.adapter.is_some()
    }
}

#[cfg(target_os = "windows")]
mod system {
    use super::*;
    use gijirec_presentation::infrastructure::audio::WindowsLoopbackAdapter;

    pub(crate) struct SystemPortAdapter {
        adapter: Option<WindowsLoopbackAdapter>,
        shared: Arc<SystemStreamShared>,
    }

    impl SystemPortAdapter {
        pub(crate) fn new(shared: Arc<SystemStreamShared>) -> Self {
            Self {
                adapter: None,
                shared,
            }
        }
    }

    impl Default for SystemPortAdapter {
        fn default() -> Self {
            let (_, _, system) = CaptureStreamHandles::new_pair();
            system
        }
    }

    impl SystemAudioCapturePort for SystemPortAdapter {
        fn open(&mut self) -> Result<(), CaptureError> {
            if self.adapter.is_some() {
                return Ok(());
            }
            match WindowsLoopbackAdapter::open(DEFAULT_RING_CAPACITY) {
                Ok((adapter, consumer, sample_rate_hz)) => {
                    *self.shared.consumer.lock().expect("lock") =
                        Some(SystemSampleConsumer::Loopback(consumer));
                    *self.shared.sample_rate_hz.lock().expect("lock") = Some(sample_rate_hz);
                    self.adapter = Some(adapter);
                    Ok(())
                }
                Err(err) => {
                    observability::log_stream_open_failure(
                        "system",
                        &err,
                        observability::session_id(),
                    );
                    Err(err)
                }
            }
        }

        fn close(&mut self) {
            *self.shared.consumer.lock().expect("lock") = None;
            *self.shared.sample_rate_hz.lock().expect("lock") = None;
            self.adapter = None;
        }

        fn is_open(&self) -> bool {
            self.adapter.is_some()
        }
    }
}

#[cfg(target_os = "macos")]
mod system {
    use super::*;
    use gijirec_presentation::infrastructure::audio::MacScreenCaptureKitAdapter;

    pub(crate) struct SystemPortAdapter {
        adapter: Option<MacScreenCaptureKitAdapter>,
        shared: Arc<SystemStreamShared>,
    }

    impl SystemPortAdapter {
        pub(crate) fn new(shared: Arc<SystemStreamShared>) -> Self {
            Self {
                adapter: None,
                shared,
            }
        }
    }

    impl Default for SystemPortAdapter {
        fn default() -> Self {
            let (_, _, system) = CaptureStreamHandles::new_pair();
            system
        }
    }

    impl SystemAudioCapturePort for SystemPortAdapter {
        fn open(&mut self) -> Result<(), CaptureError> {
            if self.adapter.is_some() {
                return Ok(());
            }
            match MacScreenCaptureKitAdapter::open(DEFAULT_RING_CAPACITY) {
                Ok((adapter, consumer, sample_rate_hz)) => {
                    *self.shared.consumer.lock().expect("lock") =
                        Some(SystemSampleConsumer::Sck(consumer));
                    *self.shared.sample_rate_hz.lock().expect("lock") = Some(sample_rate_hz);
                    self.adapter = Some(adapter);
                    Ok(())
                }
                Err(err) => {
                    observability::log_stream_open_failure(
                        "system",
                        &err,
                        observability::session_id(),
                    );
                    Err(err)
                }
            }
        }

        fn close(&mut self) {
            *self.shared.consumer.lock().expect("lock") = None;
            *self.shared.sample_rate_hz.lock().expect("lock") = None;
            self.adapter = None;
        }

        fn is_open(&self) -> bool {
            self.adapter.is_some()
        }
    }
}

#[cfg(target_os = "linux")]
mod system {
    use super::*;

    pub(crate) struct SystemPortAdapter {
        shared: Arc<SystemStreamShared>,
    }

    impl SystemPortAdapter {
        pub(crate) fn new(shared: Arc<SystemStreamShared>) -> Self {
            Self { shared }
        }
    }

    impl Default for SystemPortAdapter {
        fn default() -> Self {
            let (_, _, system) = CaptureStreamHandles::new_pair();
            system
        }
    }

    impl SystemAudioCapturePort for SystemPortAdapter {
        fn open(&mut self) -> Result<(), CaptureError> {
            let _ = &self.shared;
            Err(CaptureError::SystemAudioUnavailable)
        }

        fn close(&mut self) {
            *self.shared.consumer.lock().expect("lock") = None;
            *self.shared.sample_rate_hz.lock().expect("lock") = None;
        }

        fn is_open(&self) -> bool {
            false
        }
    }
}

pub(crate) use system::SystemPortAdapter;
