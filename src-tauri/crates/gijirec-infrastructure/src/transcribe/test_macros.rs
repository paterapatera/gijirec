//! Shared test macros for transcribe worker/engine fixtures.

/// No-op [`ModelPathLoadable`](crate::transcribe::ModelPathLoadable) for mock engines in unit tests.
#[macro_export]
macro_rules! noop_model_path_loadable {
    ($ty:ty) => {
        impl $crate::transcribe::ModelPathLoadable for $ty {
            fn load_from_path_if_needed(
                &mut self,
                _path: &std::path::Path,
            ) -> Result<(), gijirec_domain::transcribe::TranscribeError> {
                Ok(())
            }
        }
    };
}
