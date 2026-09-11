//! Shared rtrb f32 consumer helpers for mic and loopback capture adapters.

pub(crate) fn drain_f32_slots<F>(mut next: F, out: &mut [f32]) -> usize
where
    F: FnMut() -> Option<f32>,
{
    let mut count = 0;
    for slot in out.iter_mut() {
        match next() {
            Some(sample) => {
                *slot = sample;
                count += 1;
            }
            None => break,
        }
    }
    count
}

/// Consumer side of an f32 mono capture ring buffer.
pub struct F32RingConsumer {
    inner: rtrb::Consumer<f32>,
}

impl F32RingConsumer {
    pub fn pop(&mut self) -> Option<f32> {
        self.inner.pop().ok()
    }

    /// Creates a consumer from an existing rtrb queue (synthetic streams in integration tests).
    pub fn from_ring_consumer(inner: rtrb::Consumer<f32>) -> Self {
        Self { inner }
    }

    pub fn drain_into(&mut self, out: &mut [f32]) -> usize {
        drain_f32_slots(|| self.inner.pop().ok(), out)
    }

    pub fn slots(&self) -> usize {
        self.inner.slots()
    }
}
