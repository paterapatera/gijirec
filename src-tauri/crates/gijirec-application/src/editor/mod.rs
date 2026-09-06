//! Application services for the transcript editor (save paths, settings).
//!
//! `chrono` and `chrono-tz` are wired here for future JST save-path generation.

pub mod save_service;
pub mod settings_service;

pub use save_service::SaveService;
pub use settings_service::{EditorSettingsPatch, SettingsService};

#[cfg(test)]
mod chrono_deps {
    use chrono::Utc;
    use chrono_tz::Asia::Tokyo;

    #[test]
    fn jst_timezone_is_available() {
        let jst = Utc::now().with_timezone(&Tokyo);
        assert!(!jst.to_string().is_empty());
    }
}
