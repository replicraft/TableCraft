//! Original fitted requests and a distinct closed data record, never permission.
mod carrier;
mod codec;
mod model;
mod read;

pub(super) use codec::MAGIC;
pub use model::{NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates};

#[cfg(test)]
mod tests;
