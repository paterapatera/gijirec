//! Presentation crate (composition root). May depend on inner layers.
pub use gijirec_application as application;
pub use gijirec_domain as domain;
pub use gijirec_infrastructure as infrastructure;

pub mod tauri;
pub mod transcribe;
