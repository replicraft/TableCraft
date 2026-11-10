//! Original fixed-group data, separate from macro results and native authority.
mod carrier;
mod codec;
mod model;

pub use model::{NativeFixedEditIntent, NativeFixedIntentGroup};

pub(super) use codec::MAGIC;
pub(super) use model::borrow_groups;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod ordinary_tests;

#[cfg(test)]
mod ieee_tests;

#[cfg(test)]
mod scalar_tests;

#[cfg(test)]
mod guid_tests;

#[cfg(test)]
mod historical_present_tests;
