//! Segment sink trait for inference worker to application wiring.

use super::error::TranscribeError;

/// Callback from inference worker to application / block emitter.
pub trait TranscriptSegmentSink: Send + Sync {
    fn on_segment(&self, text: &str, start_ms: u64, language: &str) -> Result<(), TranscribeError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct MockSink {
        last: Arc<Mutex<Option<(String, u64, String)>>>,
    }

    impl TranscriptSegmentSink for MockSink {
        fn on_segment(
            &self,
            text: &str,
            start_ms: u64,
            language: &str,
        ) -> Result<(), TranscribeError> {
            *self.last.lock().expect("lock") =
                Some((text.to_string(), start_ms, language.to_string()));
            Ok(())
        }
    }

    #[test]
    fn segment_sink_trait_is_object_safe_and_injectable() {
        let last = Arc::new(Mutex::new(None));
        let sink: Box<dyn TranscriptSegmentSink> = Box::new(MockSink {
            last: Arc::clone(&last),
        });

        sink.on_segment("hello", 1_500, "ja")
            .expect("segment should succeed");
        assert_eq!(
            last.lock().expect("lock").clone(),
            Some(("hello".to_string(), 1_500, "ja".to_string()))
        );
    }
}
