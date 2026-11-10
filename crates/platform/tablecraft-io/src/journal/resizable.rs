//! Versioned image replacement with an explicit resizable target capability.
mod checked;
mod format;
mod images;
mod recovery;
mod transaction;

pub use checked::{
    CheckedResizableImage, CheckedV2, CheckedV3, ResizableState, V2State, V3State, inspect_v2,
    inspect_v3,
};
pub use recovery::{recover_resizable, recover_resizable_v3};
pub use transaction::{
    PreparedResizableImage, prepare_resizable_expected, prepare_resizable_v3_expected,
};

#[cfg(test)]
pub(in crate::journal) mod tests;
