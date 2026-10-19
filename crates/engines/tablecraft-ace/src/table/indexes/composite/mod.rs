//! Bounded physical tuples; compact numeric/GUID and wide Decimal models stay distinct.
mod columns;
mod decimal_columns;
mod decimal_value;
mod model;
mod policy;
mod profile;

pub(super) use columns::Columns;
pub(super) use decimal_columns::{DecimalColumns, has_decimal};
pub(in crate::table) use decimal_value::DecimalValue;
pub(in crate::table) use model::MixedValue;
pub(super) use model::{ComponentValue, CompositeKey};
pub(super) use policy::ColumnSet;
pub(super) use profile::{CompositeProfile, MAX_ENTRY_BYTES, MAX_GUID_ENTRY_BYTES};

pub(super) type GuidCompositeProfile = CompositeProfile<MixedValue>;
pub(super) type DecimalCompositeProfile =
    CompositeProfile<decimal_value::DecimalValue, DecimalColumns>;
