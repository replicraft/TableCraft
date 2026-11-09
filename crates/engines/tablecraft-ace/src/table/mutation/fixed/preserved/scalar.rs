//! Closed eight-byte scalar staging within complete mixed-schema preservation.
pub(in crate::table::mutation::fixed::preserved) mod binding;
pub(super) mod proof;
mod stage;

pub(super) use binding::{Target, bind_null_target, requested_non_null};
pub(super) use stage::{prepare, prepare_bound};
