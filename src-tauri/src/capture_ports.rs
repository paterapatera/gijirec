//! Mic and system audio infrastructure adapters as orchestrator ports.

use gijirec_presentation::application::capture::orchestrator::{
    MicCapturePort, SystemAudioCapturePort,
};
use gijirec_presentation::domain::audio::{AudioDeviceId, CaptureError};
use gijirec_presentation::infrastructure::audio::mic_capture::{
    DEFAULT_RING_CAPACITY, MicCaptureAdapter, MicSampleConsumer, StreamRuntimeErrorCallback,
};
use gijirec_presentation::tauri::observability;
use std::sync::{Arc, Mutex};

#[cfg(debug_assertions)]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(target_os = "windows")]
use gijirec_presentation::infrastructure::audio::LoopbackSampleConsumer;
#[cfg(target_os = "macos")]
use gijirec_presentation::infrastructure::audio::SckAudioSampleConsumer;

/// Callback invoked when a live capture stream reports disconnect/error (req 4.3).
pub(crate) type StreamDisconnectHandler = Arc<dyn Fn() + Send + Sync>;

pub(crate) struct StreamDisconnectSlot {
    handler: Mutex<Option<StreamDisconnectHandler>>,
}

fn stream_disconnect_runtime_hook(
    disconnect: Arc<StreamDisconnectSlot>,
) -> Option<StreamRuntimeErrorCallback> {
    Some(Arc::new(move || disconnect.notify()))
}

fn reset_stream_slots<T>(consumer: &Mutex<Option<T>>, sample_rate_hz: &Mutex<Option<u32>>) {
    *consumer.lock().expect("lock") = None;
    *sample_rate_hz.lock().expect("lock") = None;
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
#[allow(unreachable_pub)] // `src-tauri/tests/` integration harnesses consume via `test_support`
/// Shared stream handles; `pub` for `tests/` integration harnesses (`test_support`).
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

    #[cfg(debug_assertions)]
    #[allow(dead_code)]
    pub(crate) fn clear_mic(&self) {
        *self.mic.consumer.lock().expect("lock") = None;
        *self.mic.sample_rate_hz.lock().expect("lock") = None;
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

#[cfg(debug_assertions)]
struct SyntheticPortCounters {
    opened: Arc<Mutex<bool>>,
    open_count: Option<Arc<AtomicUsize>>,
    close_count: Option<Arc<AtomicUsize>>,
    producer: Option<Arc<Mutex<Option<rtrb::Producer<f32>>>>>,
}

#[cfg(debug_assertions)]
impl SyntheticPortCounters {
    fn new(opened: Arc<Mutex<bool>>) -> Self {
        Self {
            opened,
            open_count: None,
            close_count: None,
            producer: None,
        }
    }

    fn instrumented(
        opened: Arc<Mutex<bool>>,
        open_count: Arc<AtomicUsize>,
        close_count: Arc<AtomicUsize>,
        producer: Arc<Mutex<Option<rtrb::Producer<f32>>>>,
    ) -> Self {
        Self {
            opened,
            open_count: Some(open_count),
            close_count: Some(close_count),
            producer: Some(producer),
        }
    }

    fn wire_open(&self, prod: rtrb::Producer<f32>) {
        if let Some(slot) = &self.producer {
            *slot.lock().expect("lock") = Some(prod);
        }
        if let Some(count) = &self.open_count {
            count.fetch_add(1, Ordering::SeqCst);
        }
        *self.opened.lock().expect("lock") = true;
    }

    fn wire_close(&self) {
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

#[cfg(debug_assertions)]
fn wire_synthetic_ring_open(
    counters: &SyntheticPortCounters,
    wire_consumer: impl FnOnce(rtrb::Consumer<f32>),
    sample_rate_hz: &Mutex<Option<u32>>,
) -> Result<(), CaptureError> {
    use gijirec_presentation::domain::audio::pcm_chunk::SAMPLE_RATE_HZ;
    let (prod, cons) = rtrb::RingBuffer::<f32>::new(DEFAULT_RING_CAPACITY);
    wire_consumer(cons);
    *sample_rate_hz.lock().expect("lock") = Some(SAMPLE_RATE_HZ);
    counters.wire_open(prod);
    Ok(())
}

#[cfg(debug_assertions)]
macro_rules! impl_synthetic_port_struct {
    ($port:ident) => {
        #[allow(dead_code)]
        pub struct $port {
            handles: CaptureStreamHandles,
            counters: SyntheticPortCounters,
        }

        #[allow(dead_code)]
        impl $port {
            pub(crate) fn new(handles: CaptureStreamHandles, opened: Arc<Mutex<bool>>) -> Self {
                Self {
                    handles,
                    counters: SyntheticPortCounters::new(opened),
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
                    counters: SyntheticPortCounters::instrumented(
                        opened,
                        open_count,
                        close_count,
                        producer,
                    ),
                }
            }
        }
    };
}

// Mic port that installs synthetic streams into shared handles (integration tests).
#[cfg(debug_assertions)]
impl_synthetic_port_struct!(SyntheticMicPort);

#[cfg(debug_assertions)]
impl MicCapturePort for SyntheticMicPort {
    fn open(&mut self) -> Result<(), CaptureError> {
        wire_synthetic_ring_open(
            &self.counters,
            |cons| {
                *self.handles.mic.consumer.lock().expect("lock") =
                    Some(MicSampleConsumer::from_ring_consumer(cons));
            },
            &self.handles.mic.sample_rate_hz,
        )
    }

    fn close(&mut self) {
        self.handles.clear_mic();
        self.counters.wire_close();
    }

    fn is_open(&self) -> bool {
        self.counters.is_open()
    }
}

// System port that installs synthetic streams into shared handles (integration tests).
#[cfg(debug_assertions)]
impl_synthetic_port_struct!(SyntheticSystemPort);

#[cfg(debug_assertions)]
impl SystemAudioCapturePort for SyntheticSystemPort {
    fn open(&mut self) -> Result<(), CaptureError> {
        wire_synthetic_ring_open(
            &self.counters,
            |cons| install_system_consumer(&self.handles.system, cons),
            &self.handles.system.sample_rate_hz,
        )
    }

    fn close(&mut self) {
        self.handles.clear_system();
        self.counters.wire_close();
    }

    fn is_open(&self) -> bool {
        self.counters.is_open()
    }
}

/// [`MicCapturePort`] backed by [`MicCaptureAdapter`].
pub(crate) struct MicPortAdapter {
    adapter: Option<MicCaptureAdapter>,
    shared: Arc<MicStreamShared>,
    disconnect: Arc<StreamDisconnectSlot>,
}

fn adapter_stream_runtime_hook(
    disconnect: &Arc<StreamDisconnectSlot>,
) -> Option<StreamRuntimeErrorCallback> {
    stream_disconnect_runtime_hook(Arc::clone(disconnect))
}

#[allow(clippy::too_many_arguments)]
fn commit_adapter_open<C, A>(
    shared_consumer: &Mutex<Option<C>>,
    sample_rate_hz: &Mutex<Option<u32>>,
    adapter_slot: &mut Option<A>,
    open_result: Result<(A, C, u32), CaptureError>,
    port_label: &'static str,
) -> Result<(), CaptureError> {
    match open_result {
        Ok((adapter, consumer, rate)) => {
            *shared_consumer.lock().expect("lock") = Some(consumer);
            *sample_rate_hz.lock().expect("lock") = Some(rate);
            *adapter_slot = Some(adapter);
            Ok(())
        }
        Err(err) => {
            observability::log_stream_open_failure(port_label, &err, observability::session_id());
            Err(err)
        }
    }
}

fn close_adapter_port<C, A>(
    shared_consumer: &Mutex<Option<C>>,
    sample_rate_hz: &Mutex<Option<u32>>,
    adapter_slot: &mut Option<A>,
) {
    reset_stream_slots(shared_consumer, sample_rate_hz);
    *adapter_slot = None;
}

macro_rules! impl_selection_adapter_capture_port {
    (
        $trait:path,
        $ty:ty,
        |$self:ident, $device_id:ident| $open:block
    ) => {
        impl $trait for $ty {
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
                let $self = self;
                let $device_id = device_id;
                $open
            }

            fn close(&mut self) {
                close_adapter_port(
                    &self.shared.consumer,
                    &self.shared.sample_rate_hz,
                    &mut self.adapter,
                );
            }

            fn is_open(&self) -> bool {
                self.adapter.is_some()
            }
        }
    };
}

impl MicPortAdapter {
    pub(crate) fn new(shared: Arc<MicStreamShared>, disconnect: Arc<StreamDisconnectSlot>) -> Self {
        Self {
            adapter: None,
            shared,
            disconnect,
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

impl_selection_adapter_capture_port!(MicCapturePort, MicPortAdapter, |adapter, device_id| {
    commit_adapter_open(
        &adapter.shared.consumer,
        &adapter.shared.sample_rate_hz,
        &mut adapter.adapter,
        MicCaptureAdapter::open_with_device_id_and_runtime_hook(
            device_id,
            DEFAULT_RING_CAPACITY,
            adapter_stream_runtime_hook(&adapter.disconnect),
        ),
        "mic",
    )
});

macro_rules! impl_system_port_shell {
    ($adapter_ty:ty) => {
        pub(crate) struct SystemPortAdapter {
            adapter: Option<$adapter_ty>,
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
        }

        impl Default for SystemPortAdapter {
            fn default() -> Self {
                let (_, _, system) = CaptureStreamHandles::new_pair();
                system
            }
        }
    };
}

#[cfg(target_os = "windows")]
mod system {
    use super::*;
    use gijirec_presentation::infrastructure::audio::WindowsLoopbackAdapter;

    impl_system_port_shell!(WindowsLoopbackAdapter);

    impl_selection_adapter_capture_port!(
        SystemAudioCapturePort,
        SystemPortAdapter,
        |adapter, device_id| {
            commit_adapter_open(
                &adapter.shared.consumer,
                &adapter.shared.sample_rate_hz,
                &mut adapter.adapter,
                WindowsLoopbackAdapter::open_with_device_id_and_runtime_hook(
                    device_id,
                    DEFAULT_RING_CAPACITY,
                    adapter_stream_runtime_hook(&adapter.disconnect),
                )
                .map(|(adapter, consumer, sample_rate_hz)| {
                    (
                        adapter,
                        SystemSampleConsumer::Loopback(consumer),
                        sample_rate_hz,
                    )
                }),
                "system",
            )
        }
    );
}

#[cfg(target_os = "macos")]
mod system {
    use super::*;
    use gijirec_presentation::infrastructure::audio::MacScreenCaptureKitAdapter;

    impl_system_port_shell!(MacScreenCaptureKitAdapter);

    impl_selection_adapter_capture_port!(
        SystemAudioCapturePort,
        SystemPortAdapter,
        |adapter, _device_id| {
            commit_adapter_open(
                &adapter.shared.consumer,
                &adapter.shared.sample_rate_hz,
                &mut adapter.adapter,
                MacScreenCaptureKitAdapter::open_with_runtime_hook(
                    DEFAULT_RING_CAPACITY,
                    adapter_stream_runtime_hook(&adapter.disconnect),
                )
                .map(|(adapter, consumer, sample_rate_hz)| {
                    (adapter, SystemSampleConsumer::Sck(consumer), sample_rate_hz)
                }),
                "system",
            )
        }
    );
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
