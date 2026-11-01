//! Original fitted requests and a distinct closed data record, never permission.
mod carrier;
mod codec;
mod model;
mod profile;
mod read;

pub(super) use codec::MAGIC;
pub use model::{NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates};
pub(super) use profile::{OWNED_GROWTH_MAGIC, WireProfile};

#[cfg(test)]
mod tests;
