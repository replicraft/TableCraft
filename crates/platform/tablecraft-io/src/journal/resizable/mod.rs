//! Versioned image replacement with an explicit resizable target capability.
mod checked;
mod format;
mod images;
mod recovery;
mod transaction;

pub use checked::{CheckedV2, V2State, inspect_v2};
pub use recovery::recover_resizable;
pub use transaction::{PreparedResizableImage, prepare_resizable_expected};

#[cfg(test)]
pub(in crate::journal) mod tests;
