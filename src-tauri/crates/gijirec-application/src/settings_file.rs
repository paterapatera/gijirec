//! Shared JSON settings file persistence helpers.

use serde::Serialize;
use std::path::Path;

#[macro_export]
macro_rules! settings_service_shell {
    ($vis:vis $name:ident, $filename:expr) => {
        $vis struct $name {
            data_dir: std::path::PathBuf,
        }

        impl $name {
            pub fn new(data_dir: std::path::PathBuf) -> Self {
                Self { data_dir }
            }

            pub fn data_dir(&self) -> &std::path::Path {
                &self.data_dir
            }

            fn settings_path(&self) -> std::path::PathBuf {
                self.data_dir.join($filename)
            }
        }
    };
}

pub(crate) fn write_json_pretty<T: Serialize, E>(
    path: &Path,
    value: &T,
    persist_failed: fn(String) -> E,
) -> Result<(), E> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| persist_failed(format!("create settings directory: {err}")))?;
    }

    let json = serde_json::to_string_pretty(value)
        .map_err(|err| persist_failed(format!("serialize settings: {err}")))?;
    std::fs::write(path, json).map_err(|err| persist_failed(format!("write settings: {err}")))?;
    Ok(())
}
