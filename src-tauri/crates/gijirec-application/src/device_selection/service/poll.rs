//! Hotplug polling state and background thread lifecycle.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use gijirec_domain::audio::AudioDeviceList;

use super::{
    DefaultDeviceSelectionService, DeviceEnumeratorPort, DeviceSelectionClock,
    DeviceSelectionError, DeviceSelectionEvents, HOTPLUG_POLL_INTERVAL_MS,
};

pub(crate) struct PollState {
    pub(crate) ui_visible: bool,
    last_poll_ms: u64,
    last_snapshot: Option<AudioDeviceList>,
}

pub(crate) struct PollShared<E, Ev, C> {
    pub(crate) enumerator: E,
    pub(crate) events: Ev,
    pub(crate) clock: C,
    pub(crate) poll: Mutex<PollState>,
}

impl<E, Ev, C> PollShared<E, Ev, C>
where
    E: DeviceEnumeratorPort,
    Ev: DeviceSelectionEvents,
    C: DeviceSelectionClock,
{
    pub(crate) fn poll_devices_if_due(&self, force: bool) -> Result<(), DeviceSelectionError> {
        let mut poll = self
            .poll
            .lock()
            .map_err(|_| DeviceSelectionError::internal("poll lock poisoned"))?;
        if !poll.ui_visible {
            return Ok(());
        }

        let now = self.clock.now_ms();
        if !force && now.saturating_sub(poll.last_poll_ms) < HOTPLUG_POLL_INTERVAL_MS {
            return Ok(());
        }

        let devices = self.enumerator.list_devices()?;
        poll.last_poll_ms = now;

        let changed = poll.last_snapshot.as_ref() != Some(&devices);
        if changed {
            poll.last_snapshot = Some(devices.clone());
            self.events.emit_devices_changed(&devices, now);
        }
        Ok(())
    }
}

pub(crate) fn new_poll_state() -> PollState {
    PollState {
        ui_visible: false,
        last_poll_ms: 0,
        last_snapshot: None,
    }
}

fn run_hotplug_poll_loop<E, Ev, C>(shared: Arc<PollShared<E, Ev, C>>, stop: Arc<AtomicBool>)
where
    E: DeviceEnumeratorPort + 'static,
    Ev: DeviceSelectionEvents + 'static,
    C: DeviceSelectionClock + 'static,
{
    loop {
        let interval_ms = shared.clock.hotplug_poll_interval_ms();
        sleep_poll_interval(&stop, interval_ms);
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let ui_visible = shared
            .poll
            .lock()
            .map(|poll| poll.ui_visible)
            .unwrap_or(false);
        if !ui_visible {
            break;
        }
        let _ = shared.poll_devices_if_due(false);
    }
}

fn sleep_poll_interval(stop: &AtomicBool, interval_ms: u64) {
    const CHUNK_MS: u64 = 50;
    let mut elapsed = 0;
    while elapsed < interval_ms && !stop.load(Ordering::Relaxed) {
        let step = CHUNK_MS.min(interval_ms - elapsed);
        thread::sleep(Duration::from_millis(step));
        elapsed += step;
    }
}

pub(crate) fn stop_poll_thread<E, O, P, Ev, C>(
    service: &DefaultDeviceSelectionService<E, O, P, Ev, C>,
) {
    service.poll_stop.store(true, Ordering::Relaxed);
    if let Ok(mut guard) = service.poll_thread.lock()
        && let Some(handle) = guard.take()
    {
        let _ = handle.join();
    }
    service.poll_stop.store(false, Ordering::Relaxed);
}

pub(crate) fn start_poll_thread<E, O, P, Ev, C>(
    service: &DefaultDeviceSelectionService<E, O, P, Ev, C>,
) where
    E: DeviceEnumeratorPort + 'static,
    Ev: DeviceSelectionEvents + 'static,
    C: DeviceSelectionClock + 'static,
{
    stop_poll_thread(service);
    service.poll_stop.store(false, Ordering::Relaxed);
    let shared = Arc::clone(&service.shared);
    let stop = Arc::clone(&service.poll_stop);
    let handle = thread::spawn(move || run_hotplug_poll_loop(shared, stop));
    if let Ok(mut guard) = service.poll_thread.lock() {
        *guard = Some(handle);
    }
}
