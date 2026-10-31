pub(in crate::table) mod batch;
mod cascade;
mod decimal;
mod encoding;
pub(super) mod extended;
pub(in crate::table::mutation) mod masked;
pub(in crate::table::mutation) mod preserved;
mod preview;
mod update;

#[cfg(all(test, feature = "ace16-write-preview"))]
mod historical_columns;

#[cfg(test)]
mod ignored_updates;

pub use batch::FixedColumnUpdates;
pub use update::FixedScalarUpdate;

pub(in crate::table::mutation) use encoding::encode as encode_fixed;
pub(in crate::table::mutation) use update::EncodedUpdate;
