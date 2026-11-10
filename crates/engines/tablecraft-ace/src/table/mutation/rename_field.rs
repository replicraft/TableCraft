//! Same-width saved-field rename construction over a sealed complete OFF source.
mod candidate;
mod error;
mod fitted;
mod identity;
mod populated_source;
mod source;
#[cfg(test)]
mod tests;
mod verify;

pub use candidate::NativeFieldRenameCandidate;
pub use error::FieldRenameError;
pub use fitted::{
    FittedFieldRenameSource, NativeFittedFieldRenameCandidate, NativePopulatedFieldRenameCandidate,
};
pub use identity::NativeFieldRenameIdentity;
