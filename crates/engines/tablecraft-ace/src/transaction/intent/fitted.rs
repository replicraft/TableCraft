//! Original fitted requests and a distinct closed data record, never permission.
pub(super) mod carrier;
mod codec;
mod model;
mod profile;
mod read;

pub(super) use codec::MAGIC;
pub use model::{NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates};
pub(super) use profile::{
    CURRENT_ROWS_MAGIC, EOF_ALLOCATION_MAGIC, HISTORICAL_TEXT_MAGIC, OWNED_GROWTH_MAGIC,
    WireProfile,
};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod allocation_tests;

#[cfg(all(test, feature = "ace16-create"))]
mod historical_text_tests;

#[cfg(all(test, feature = "ace16-create"))]
mod current_rows_tests;
