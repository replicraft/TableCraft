//! Versioned image replacement with an explicit resizable target capability.
mod format;
mod images;
mod recovery;
mod transaction;

pub use recovery::recover_resizable;
pub use transaction::{PreparedResizableImage, prepare_resizable_expected};

#[cfg(test)]
pub(in crate::journal) mod tests;
