//! Original fitted requests and a distinct closed data record, never permission.
pub(super) mod carrier;
mod codec;
mod fixed_scalar_carrier;
mod grammar;
mod model;
mod profile;
mod read;
mod wide_value_carrier;

pub(super) use codec::MAGIC;
pub use model::{NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates};
pub(super) use profile::{
    CURRENT_ROWS_MAGIC, EOF_ALLOCATION_MAGIC, HISTORICAL_TEXT_MAGIC, OWNED_GROWTH_MAGIC,
    RETAINED_BOOLEAN_MATERIALIZATION_MAGIC, RETAINED_BOOLEAN_ROWS_MAGIC,
    RETAINED_CURRENCY_COLUMN_BATCH_MAGIC, RETAINED_CURRENCY_ROWS_MAGIC, RETAINED_EMPTY_ROWS_MAGIC,
    RETAINED_FIXED_SCALAR_ROWS_MAGIC, RETAINED_INDEXED_LONG_MAGIC,
    RETAINED_LONG_COLUMN_BATCH_MAGIC, RETAINED_OPAQUE_ROWS_MAGIC, RETAINED_ROWS_MAGIC,
    RETAINED_WIDE_VALUE_ROWS_MAGIC, WireProfile,
};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod allocation_tests;

#[cfg(all(test, feature = "ace16-create"))]
mod historical_text_tests;

#[cfg(all(test, feature = "ace16-create"))]
mod current_rows_tests;

#[cfg(test)]
mod retained_tests;

#[cfg(test)]
mod opaque_tests;

#[cfg(test)]
mod currency_tests;

#[cfg(test)]
mod boolean_tests;

#[cfg(test)]
mod fixed_scalar_tests;

#[cfg(test)]
mod wide_value_tests;

#[cfg(test)]
mod boolean_materialization_tests;

#[cfg(test)]
mod empty_tests;

#[cfg(test)]
mod long_column_batch_tests;

#[cfg(test)]
mod indexed_long_tests;
