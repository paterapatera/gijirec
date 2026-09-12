pub(crate) use std::sync::Arc;
pub(crate) use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
pub(crate) use std::thread::{self, JoinHandle};
pub(crate) use std::time::{Duration, Instant};

pub(crate) use gijirec_domain::transcribe::{TranscribeError, TranscriptSegmentSink};
