//! Saved nonprimary field removal over a complete settled explicit-OFF source.
mod candidate;
mod definition;
mod error;
mod identity;
mod selection;
mod source;
#[cfg(test)]
mod tests;
mod verify;

pub use candidate::NativeFieldDropCandidate;
pub use error::FieldDropError;
pub use identity::NativeFieldDropIdentity;
pub use selection::SavedFieldDropSourcePurpose;
