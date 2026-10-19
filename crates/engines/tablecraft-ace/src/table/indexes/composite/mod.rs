//! Bounded physical tuples; compact numeric/GUID and wide Decimal models stay distinct.
mod columns;
mod decimal_value;
mod model;
mod profile;

pub(super) use columns::Columns;
pub(super) use model::MixedValue;
pub(super) use profile::{CompositeProfile, MAX_ENTRY_BYTES, MAX_GUID_ENTRY_BYTES};

pub(super) type GuidCompositeProfile = CompositeProfile<MixedValue>;
pub(super) type DecimalCompositeProfile = CompositeProfile<decimal_value::DecimalValue>;
