pub mod injector;
#[cfg(target_os = "macos")]
pub mod macos_pasteboard;

pub use injector::paste_text_to_active_app;
