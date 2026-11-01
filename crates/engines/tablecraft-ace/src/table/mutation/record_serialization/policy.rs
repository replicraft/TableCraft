//! Explicit operation-owned wire policies; caches never select a proof policy.
#[derive(Clone, Copy)]
pub(in crate::table::mutation) enum RecordPolicy {
    FittedDeclared,
    GrowingPreferred,
}
