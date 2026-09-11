//! Shared stop-signal and thread join helpers.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

pub(crate) fn signal_stop_and_join_thread(stop: &AtomicBool, thread: &mut Option<JoinHandle<()>>) {
    stop.store(true, Ordering::SeqCst);
    if let Some(handle) = thread.take() {
        let _ = handle.join();
    }
}
