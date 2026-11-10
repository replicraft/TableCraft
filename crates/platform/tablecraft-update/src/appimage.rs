//! Publisher-authenticated original AppImages and whole-image update preparation.

pub(crate) mod context;
mod installed;
mod payload;
mod preparation;

pub(crate) use context::{RuntimeContext, capture};
pub use installed::InstalledAppImage;
pub(crate) use installed::authenticate;
pub(crate) use preparation::PreparedAppImage;
