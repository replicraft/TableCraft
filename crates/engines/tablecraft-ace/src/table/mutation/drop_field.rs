//! Saved nonprimary field removal over a complete settled explicit-OFF source.
mod candidate;
mod current_tail;
mod definition;
mod error;
mod identity;
mod non_tail;
#[cfg(test)]
mod retained_middle_tests;
mod schema_candidate;
mod schema_source;
mod schema_verify;
mod selection;
mod source;
#[cfg(test)]
mod tests;
mod verify;

pub use candidate::NativeFieldDropCandidate;
pub use current_tail::NativeCurrentTailFieldDropCandidate;
pub use error::FieldDropError;
pub use identity::NativeFieldDropIdentity;
pub use non_tail::NativeRetainedNonTailFieldDropCandidate;
pub use selection::SavedFieldDropSourcePurpose;
