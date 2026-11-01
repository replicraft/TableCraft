//! Original presence resolution shared below preview and normal packaging.
pub(in crate::table::mutation) mod clock;
pub(in crate::table::mutation) mod literal;
pub(in crate::table::mutation) mod proof_literal;
pub(in crate::table::mutation) mod values;

#[cfg(feature = "journal")]
mod resolve;

#[cfg(feature = "journal")]
pub(in crate::table::mutation) use resolve::{derive, prepare};
