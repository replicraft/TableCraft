//! Journal protocols retain independent original-action qualification owners.
#[cfg(feature = "ace16-create")]
mod legacy;
mod retained;

#[cfg(feature = "ace16-create")]
pub use legacy::{QualifiedFieldDropRecovery, qualify_native_field_drop_recovery};
pub use retained::QualifiedRetainedFieldDropRecovery;
