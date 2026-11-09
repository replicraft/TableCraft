//! Fresh canonical ACE 16 creation and explicitly required publication.
mod error;
mod service;

#[cfg(all(test, any(unix, windows)))]
mod tests;

pub use error::CreationError;
pub use service::{CreationDestination, CreationReceipt, PreparedCreation};
