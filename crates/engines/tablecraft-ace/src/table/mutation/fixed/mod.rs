mod batch;
mod cascade;
mod decimal;
mod encoding;
pub(in crate::table::mutation) mod masked;
mod preview;
mod update;

pub use batch::FixedColumnUpdates;
pub use update::FixedScalarUpdate;

pub(in crate::table::mutation) use encoding::encode as encode_fixed;
pub(in crate::table::mutation) use update::EncodedUpdate;
