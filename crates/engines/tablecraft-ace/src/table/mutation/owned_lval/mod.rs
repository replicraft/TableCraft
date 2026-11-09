//! Shared native owned-page/maps append framing; callers retain source admission.
pub(super) mod maps;
pub(super) mod placement;

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(super) mod property;
