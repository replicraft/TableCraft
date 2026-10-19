//! Bounded physical tuples; numeric and GUID-bearing layouts stay distinct.
mod columns;
mod model;
mod profile;

pub(super) use columns::Columns;
pub(super) use model::MixedValue;
pub(super) use profile::{CompositeProfile, MAX_ENTRY_BYTES, MAX_GUID_ENTRY_BYTES};

pub(super) type GuidCompositeProfile = CompositeProfile<MixedValue>;
