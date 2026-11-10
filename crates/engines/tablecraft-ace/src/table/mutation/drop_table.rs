//! Source-bound logical table deletion with exact retained storage preservation.
#[cfg(feature = "ace16-create")]
mod current;

mod error;
mod identity;
mod plan;
mod source;
#[cfg(test)]
mod tests;
mod verify;

pub use error::TableDropError;
pub use identity::NativeTableDropIdentity;
pub use plan::{NativeTableDropInspection, NativeTableDropPlan};
pub(crate) use source::SourceProfile;
