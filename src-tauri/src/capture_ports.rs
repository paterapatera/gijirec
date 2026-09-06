//! Mic and system audio infrastructure adapters as orchestrator ports.

use gijirec_presentation::application::capture::orchestrator::{
    MicCapturePort, SystemAudioCapturePort,
};
use gijirec_presentation::domain::audio::{AudioDeviceId, CaptureError};
use gijirec_presentation::infrastructure::audio::mic_capture::{
    DEFAULT_RING_CAPACITY, MicCaptureAdapter, MicSampleConsumer,
};
use gijirec_presentation::tauri::observability;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[cfg(target_os = "windows")]
use gijirec_presentation::infrastructure::audio::LoopbackSampleConsumer;
#[cfg(target_os = "macos")]
use gijirec_presentation::infrastructure::audio::SckAudioSampleConsumer;

/// Callback invoked when a live capture stream reports disconnect/error (req 4.3).
pub(crate) type StreamDisconnectHandler = Arc<dyn Fn() + Send + Sync>;

pub(crate) struct StreamDisconnectSlot {
    handler: Mutex<Option<StreamDisconnectHandler>>,
}

impl StreamDisconnectSlot {
    fn new() -> Self {
        Self {
            handler: Mutex::new(None),
        }
    }

    fn set_handler(&self, handler: StreamDisconnectHandler) {
        *self.handler.lock().expect("lock") = Some(handler);
    }

    fn notify(&self) {
        if let Some(handler) = self.handler.lock().expect("lock").clone() {
            handler();
        }
    }
}

pub(crate) struct MicStreamShared {
    consumer: Mutex<Option<MicSampleConsumer>>,
    sample_rate_hz: Mutex<Option<u32>>,
}

/// Platform-specific system audio consumer held for the processing thread.
pub(crate) enum SystemSampleConsumer {
    #[cfg(target_os = "windows")]
    Loopback(LoopbackSampleConsumer),
    #[cfg(target_os = "macos")]
    Sck(SckAudioSampleConsumer),
    #[cfg(target_os = "linux")]
    Unavailable,
}

pub(crate) struct SystemStreamShared {
    consumer: Mutex<Option<SystemSampleConsumer>>,
    sample_rate_hz: Mutex<Option<u32>>,
}

/// Shared handles for handing rtrb consumers to the processing thread after open.
#[derive(Clone)]
pub struct CaptureStreamHandles {
    mic: Arc<MicStreamShared>,
    system: Arc<SystemStreamShared>,
    disconnect: Arc<StreamDisconnectSlot>,
}

impl CaptureStreamHandles {
    pub(crate) fn new_pair() -> (Self, MicPortAdapter, SystemPortAdapter) {
        let disconnect = Arc::new(StreamDisconnectSlot::new());
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
            disconnect: Arc::clone(&disconnect),
        };
        (
            handles,
            MicPortAdapter::new(mic, Arc::clone(&disconnect)),
            SystemPortAdapter::new(system, Arc::clone(&disconnect)),
        )
    }

    pub(crate) fn set_stream_disconnect_handler(&self, handler: StreamDisconnectHandler) {
        self.disconnect.set_handler(handler);
    }

    #[cfg(debug_assertions)]
    pub(crate) fn notify_stream_disconnected(&self) {
        self.disconnect.notify();
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
    #[cfg(debug_assertions)]
    #[allow(dead_code)]
    pub(crate) fn install_synthetic_mic(&self, sample_rate_hz: u32) {
        let (_prod, cons) = rtrb::RingBuffer::<f32>::new(DEFAULT_RING_CAPACITY);
        *self.mic.consumer.lock().expect("lock") =
            Some(MicSampleConsumer::from_ring_consumer(cons));
        *self.mic.sample_rate_hz.lock().expect("lock") = Some(sample_rate_hz);
    }

    #[cfg(debug_assertions)]
    #[allow(dead_code)]
    pub(crate) fn clear_mic(&self) {
        *self.mic.consumer.lock().expect("lock") = None;
        *self.mic.sample_rate_hz.lock().expect("lock") = None;
    }

    #[cfg(debug_assertions)]
    #[allow(dead_code)]
    pub(crate) fn install_synthetic_system(&self, sample_rate_hz: u32) {
        let (_prod, cons) = rtrb::RingBuffer::<f32>::new(DEFAULT_RING_CAPACITY);
        install_system_consumer(&self.system, cons);
        *self.system.sample_rate_hz.lock().expect("lock") = Some(sample_rate_hz);
    }

    #[cfg(debug_assertions)]
    #[allow(dead_code)]
    pub(crate) fn clear_system(&self) {
        *self.system.consumer.lock().expect("lock") = None;
        *self.system.sample_rate_hz.lock().expect("lock") = None;
    }
}

#[cfg(debug_assertions)]
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
#[cfg(debug_assertions)]
#[allow(dead_code)]
pub struct SyntheticMicPort {
    handles: CaptureStreamHandles,
    opened: Arc<Mutex<bool>>,
    open_count: Option<Arc<AtomicUsize>>,
    close_count: Option<Arc<AtomicUsize>>,
    producer: Option<Arc<Mutex<Option<rtrb::Producer<f32>>>>>,
}

#[cfg(debug_assertions)]
#[allow(dead_code)]
impl SyntheticMicPort {
    pub(crate) fn new(handles: CaptureStreamHandles, opened: Arc<Mutex<bool>>) -> Self {
        Self {
            handles,
            opened,
            open_count: None,
            close_count: None,
            producer: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_instrumented(
        handles: CaptureStreamHandles,
        opened: Arc<Mutex<bool>>,
        open_count: Arc<AtomicUsize>,
        close_count: Arc<AtomicUsize>,
        producer: Arc<Mutex<Option<rtrb::Producer<f32>>>>,
    ) -> Self {
        Self {
            handles,
            opened,
            open_count: Some(open_count),
            close_count: Some(close_count),
            producer: Some(producer),
        }
    }
}

#[cfg(debug_assertions)]
impl MicCapturePort for SyntheticMicPort {
    fn open(&mut self) -> Result<(), CaptureError> {
        use gijirec_presentation::domain::audio::pcm_chunk::SAMPLE_RATE_HZ;
        let (prod, cons) = rtrb::RingBuffer::<f32>::new(DEFAULT_RING_CAPACITY);
        *self.handles.mic.consumer.lock().expect("lock") =
            Some(MicSampleConsumer::from_ring_consumer(cons));
        *self.handles.mic.sample_rate_hz.lock().expect("lock") = Some(SAMPLE_RATE_HZ);
        if let Some(slot) = &self.producer {
            *slot.lock().expect("lock") = Some(prod);
        }
        if let Some(count) = &self.open_count {
            count.fetch_add(1, Ordering::SeqCst);
        }
        *self.opened.lock().expect("lock") = true;
        Ok(())
    }

    fn close(&mut self) {
        self.handles.clear_mic();
        if let Some(count) = &self.close_count {
            count.fetch_add(1, Ordering::SeqCst);
        }
        if let Some(slot) = &self.producer {
            *slot.lock().expect("lock") = None;
        }
        *self.opened.lock().expect("lock") = false;
    }

    fn is_open(&self) -> bool {
        *self.opened.lock().expect("lock")
    }
}

/// System port that installs synthetic streams into shared handles (integration tests).
#[cfg(debug_assertions)]
#[allow(dead_code)]
pub struct SyntheticSystemPort {
    handles: CaptureStreamHandles,
    opened: Arc<Mutex<bool>>,
    open_count: Option<Arc<AtomicUsize>>,
    close_count: Option<Arc<AtomicUsize>>,
    producer: Option<Arc<Mutex<Option<rtrb::Producer<f32>>>>>,
}

#[cfg(debug_assertions)]
#[allow(dead_code)]
impl SyntheticSystemPort {
    pub(crate) fn new(handles: CaptureStreamHandles, opened: Arc<Mutex<bool>>) -> Self {
        Self {
            handles,
            opened,
            open_count: None,
            close_count: None,
            producer: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_instrumented(
        handles: CaptureStreamHandles,
        opened: Arc<Mutex<bool>>,
        open_count: Arc<AtomicUsize>,
        close_count: Arc<AtomicUsize>,
        producer: Arc<Mutex<Option<rtrb::Producer<f32>>>>,
    ) -> Self {
        Self {
            handles,
            opened,
            open_count: Some(open_count),
            close_count: Some(close_count),
            producer: Some(producer),
        }
    }
}

#[cfg(debug_assertions)]
impl SystemAudioCapturePort for SyntheticSystemPort {
    fn open(&mut self) -> Result<(), CaptureError> {
        use gijirec_presentation::domain::audio::pcm_chunk::SAMPLE_RATE_HZ;
        let (prod, cons) = rtrb::RingBuffer::<f32>::new(DEFAULT_RING_CAPACITY);
        install_system_consumer(&self.handles.system, cons);
        *self.handles.system.sample_rate_hz.lock().expect("lock") = Some(SAMPLE_RATE_HZ);
        if let Some(slot) = &self.producer {
            *slot.lock().expect("lock") = Some(prod);
        }
        if let Some(count) = &self.open_count {
            count.fetch_add(1, Ordering::SeqCst);
        }
        *self.opened.lock().expect("lock") = true;
        Ok(())
    }

    fn close(&mut self) {
        self.handles.clear_system();
        if let Some(count) = &self.close_count {
            count.fetch_add(1, Ordering::SeqCst);
        }
        if let Some(slot) = &self.producer {
            *slot.lock().expect("lock") = None;
        }
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
    disconnect: Arc<StreamDisconnectSlot>,
}

impl MicPortAdapter {
    pub(crate) fn new(shared: Arc<MicStreamShared>, disconnect: Arc<StreamDisconnectSlot>) -> Self {
        Self {
            adapter: None,
            shared,
            disconnect,
        }
    }

    fn stream_runtime_hook(
        &self,
    ) -> Option<gijirec_presentation::infrastructure::audio::mic_capture::StreamRuntimeErrorCallback>
    {
        let disconnect = Arc::clone(&self.disconnect);
        Some(Arc::new(move || disconnect.notify()))
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
        self.open_with_selection(None)
    }

    fn open_with_selection(
        &mut self,
        device_id: Option<&AudioDeviceId>,
    ) -> Result<(), CaptureError> {
        if self.adapter.is_some() {
            return Ok(());
        }
        match MicCaptureAdapter::open_with_device_id_and_runtime_hook(
            device_id,
            DEFAULT_RING_CAPACITY,
            self.stream_runtime_hook(),
        ) {
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
        disconnect: Arc<StreamDisconnectSlot>,
    }

    impl SystemPortAdapter {
        pub(crate) fn new(
            shared: Arc<SystemStreamShared>,
            disconnect: Arc<StreamDisconnectSlot>,
        ) -> Self {
            Self {
                adapter: None,
                shared,
                disconnect,
            }
        }

        fn stream_runtime_hook(
            &self,
        ) -> Option<
            gijirec_presentation::infrastructure::audio::mic_capture::StreamRuntimeErrorCallback,
        > {
            let disconnect = Arc::clone(&self.disconnect);
            Some(Arc::new(move || disconnect.notify()))
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
            self.open_with_selection(None)
        }

        fn open_with_selection(
            &mut self,
            device_id: Option<&AudioDeviceId>,
        ) -> Result<(), CaptureError> {
            if self.adapter.is_some() {
                return Ok(());
            }
            match WindowsLoopbackAdapter::open_with_device_id_and_runtime_hook(
                device_id,
                DEFAULT_RING_CAPACITY,
                self.stream_runtime_hook(),
            ) {
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
        disconnect: Arc<StreamDisconnectSlot>,
    }

    impl SystemPortAdapter {
        pub(crate) fn new(
            shared: Arc<SystemStreamShared>,
            disconnect: Arc<StreamDisconnectSlot>,
        ) -> Self {
            Self {
                adapter: None,
                shared,
                disconnect,
            }
        }

        fn stream_runtime_hook(
            &self,
        ) -> Option<
            gijirec_presentation::infrastructure::audio::mic_capture::StreamRuntimeErrorCallback,
        > {
            let disconnect = Arc::clone(&self.disconnect);
            Some(Arc::new(move || disconnect.notify()))
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
            match MacScreenCaptureKitAdapter::open_with_runtime_hook(
                DEFAULT_RING_CAPACITY,
                self.stream_runtime_hook(),
            ) {
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
        #[expect(dead_code)]
        disconnect: Arc<StreamDisconnectSlot>,
    }

    impl SystemPortAdapter {
        pub(crate) fn new(
            shared: Arc<SystemStreamShared>,
            disconnect: Arc<StreamDisconnectSlot>,
        ) -> Self {
            Self { shared, disconnect }
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
