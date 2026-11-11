//! Original presence resolution shared below preview and normal packaging.
pub(in crate::table::mutation) mod clock;
pub(in crate::table::mutation) mod literal;
pub(crate) mod proof_literal;
pub(in crate::table::mutation) mod values;

#[cfg(feature = "journal")]
mod resolve;

#[cfg(feature = "journal")]
pub(in crate::table::mutation) mod declaration;

#[cfg(feature = "journal")]
pub(in crate::table::mutation) use resolve::{derive, prepare};

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(in crate::table::mutation) use resolve::{derive_retained, prepare_retained};
