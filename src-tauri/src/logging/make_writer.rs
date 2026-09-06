//! Release log file writer wrapper that surfaces persistence write failures.

use super::persistence::ReleaseLogInitError;
use super::tracing_init::surface_persistence_failure;
use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing_subscriber::fmt::MakeWriter;

/// Wraps a [`MakeWriter`] and surfaces the first write I/O failure via diagnostics.
pub(crate) struct ReleaseLogMakeWriter<W> {
    inner: W,
    surfaced: Arc<AtomicBool>,
}

impl<W> ReleaseLogMakeWriter<W> {
    pub(crate) fn new(inner: W) -> Self {
        Self {
            inner,
            surfaced: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn surfaced_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.surfaced)
    }

    fn surface_write_failure_once(surfaced: &AtomicBool, err: &io::Error) {
        if surfaced.swap(true, Ordering::SeqCst) {
            return;
        }
        surface_persistence_failure(&ReleaseLogInitError::new(format!(
            "release log write failed: {err}"
        )));
    }
}

pub(crate) struct ReleaseLogWriter<W> {
    inner: W,
    surfaced: Arc<AtomicBool>,
}

impl<'a, W> MakeWriter<'a> for ReleaseLogMakeWriter<W>
where
    W: MakeWriter<'a>,
{
    type Writer = ReleaseLogWriter<W::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        ReleaseLogWriter {
            inner: self.inner.make_writer(),
            surfaced: Arc::clone(&self.surfaced),
        }
    }
}

impl<W: Write> Write for ReleaseLogWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.inner.write(buf) {
            Ok(n) => Ok(n),
            Err(err) => {
                ReleaseLogMakeWriter::<W>::surface_write_failure_once(&self.surfaced, &err);
                Err(err)
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.inner.flush() {
            Ok(()) => Ok(()),
            Err(err) => {
                ReleaseLogMakeWriter::<W>::surface_write_failure_once(&self.surfaced, &err);
                Err(err)
            }
        }
    }
}

#[cfg(test)]
pub(crate) struct AlwaysFailingWriter;

#[cfg(test)]
impl Write for AlwaysFailingWriter {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("disk full during session"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("disk full during session"))
    }
}

#[cfg(test)]
pub(crate) struct AlwaysFailingMakeWriter;

#[cfg(test)]
impl<'a> MakeWriter<'a> for AlwaysFailingMakeWriter {
    type Writer = AlwaysFailingWriter;

    fn make_writer(&'a self) -> Self::Writer {
        AlwaysFailingWriter
    }
}
