//! Shared bounded-queue delivery helpers for downstream consumer buses.

use std::sync::{Arc, Mutex};

pub(crate) enum ConsumerDeliverOutcome<T> {
    Consumed,
    Retain(T),
    Stop(T),
}

pub(crate) fn clone_registered_consumer<C: ?Sized>(slot: &Mutex<Option<Arc<C>>>) -> Option<Arc<C>> {
    slot.lock().expect("lock").clone()
}

pub(crate) fn flush_consumer_queue<T, C: ?Sized, F>(
    queue: &mut Vec<T>,
    consumer: &Arc<C>,
    mut deliver: F,
) where
    F: FnMut(&Arc<C>, T) -> ConsumerDeliverOutcome<T>,
{
    let mut remaining = Vec::new();
    for item in queue.drain(..) {
        match deliver(consumer, item) {
            ConsumerDeliverOutcome::Consumed => {}
            ConsumerDeliverOutcome::Retain(item) => remaining.push(item),
            ConsumerDeliverOutcome::Stop(item) => {
                remaining.push(item);
                break;
            }
        }
    }
    *queue = remaining;
}

pub(crate) fn flush_registered_consumer_queue<T, C: ?Sized, F>(
    consumer_slot: &Mutex<Option<Arc<C>>>,
    queue_slot: &Mutex<Vec<T>>,
    deliver: F,
) where
    F: FnMut(&Arc<C>, T) -> ConsumerDeliverOutcome<T>,
{
    let Some(consumer) = clone_registered_consumer(consumer_slot) else {
        return;
    };
    let mut queue = queue_slot.lock().expect("lock");
    flush_consumer_queue(&mut queue, &consumer, deliver);
}
