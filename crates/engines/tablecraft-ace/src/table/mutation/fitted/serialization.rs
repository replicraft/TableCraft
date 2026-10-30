//! Explicit operation-owned wire policies; caches never select a proof policy.
#[derive(Clone, Copy)]
pub(super) enum RecordPolicy {
    FittedDeclared,
    GrowingPreferred,
}
