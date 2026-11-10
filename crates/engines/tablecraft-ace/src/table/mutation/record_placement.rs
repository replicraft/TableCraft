//! Shared owned DATA placement; admission stays with each actual caller.
pub(super) mod audit;
pub(super) mod forward;
pub(super) mod slots;
pub(super) mod space;

#[cfg(feature = "journal")]
pub(super) mod owned;
#[cfg(feature = "journal")]
mod owned_audit;

pub(super) mod empty;
#[cfg(feature = "journal")]
pub(super) mod future;

#[cfg(feature = "journal")]
pub(super) mod future_pages;

#[cfg(feature = "journal")]
pub(in crate::table::mutation) mod proof;
