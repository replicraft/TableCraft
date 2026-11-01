//! Shared owned DATA placement; admission stays with each actual caller.
pub(super) mod audit;
pub(super) mod forward;
mod slots;
pub(super) mod space;

#[cfg(feature = "journal")]
pub(super) mod owned;
#[cfg(feature = "journal")]
mod owned_audit;
