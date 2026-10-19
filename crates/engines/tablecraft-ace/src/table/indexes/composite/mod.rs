//! Bounded physical tuples; compact numeric/GUID and wide Decimal models stay distinct.
mod columns;
mod decimal_columns;
mod decimal_value;
mod model;
mod policy;
mod profile;

pub(super) use columns::Columns;
pub(super) use decimal_columns::{DecimalColumns, has_decimal};
pub(super) use model::{CompositeKey, MixedValue};
pub(super) use profile::{CompositeProfile, MAX_ENTRY_BYTES, MAX_GUID_ENTRY_BYTES};

pub(super) type GuidCompositeProfile = CompositeProfile<MixedValue>;
pub(super) type DecimalCompositeProfile =
    CompositeProfile<decimal_value::DecimalValue, DecimalColumns>;
