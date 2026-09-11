//! Shared error display helpers for application-layer user-facing errors.

macro_rules! impl_message_ja_error_display {
    ($ty:ty) => {
        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}: {}", self.code.as_str(), self.message_ja)
            }
        }

        impl std::error::Error for $ty {}
    };
}

pub(crate) use impl_message_ja_error_display;
