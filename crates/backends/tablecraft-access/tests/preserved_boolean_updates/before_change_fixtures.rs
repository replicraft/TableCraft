//! One pure fixture closure over unchanged shared native owners.
use super::{
    fixed_scalar_model as scalar_model, int32_model, joint_fixed_model as fixed_model, raw, rich,
};

// Provenance/framing fields are consumed by the actual native SOURCE qualifiers.
#[allow(dead_code)]
#[path = "../../../../engines/tablecraft-ace/src/table/mutation/fixed/preserved/rich_before_change_sources/model.rs"]
mod sources;

// Provenance/framing fields are consumed by the actual native SOURCE qualifiers.
#[allow(dead_code)]
#[path = "../../../../engines/tablecraft-ace/src/table/mutation/fixed/preserved/rich_before_change_sources/required/model.rs"]
mod required_sources;

// Provenance/framing fields are consumed by the actual native SOURCE qualifiers.
#[allow(dead_code)]
#[path = "../../../../engines/tablecraft-ace/src/table/mutation/fixed/preserved/rich_before_change_sources/expectations/model.rs"]
pub(super) mod model;
