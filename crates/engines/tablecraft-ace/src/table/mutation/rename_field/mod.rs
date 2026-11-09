//! Same-width saved-field rename construction over a sealed complete OFF source.
mod candidate;
mod error;
mod identity;
mod source;
#[cfg(test)]
mod tests;
mod verify;

pub use candidate::NativeFieldRenameCandidate;
pub use error::FieldRenameError;
pub use identity::NativeFieldRenameIdentity;
