#![cfg(feature = "ace16-write-preview")]
//! Ordinary Memo/OLE source gates and explicit engineering copy consumers.
#[path = "../../../engines/tablecraft-ace/tests/ace16_long_value_updates_contract/routing_cases.rs"]
mod cases;
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
pub mod fixture;
#[path = "../../../engines/tablecraft-ace/tests/ace16_long_value_updates_contract/model.rs"]
pub mod original;

use std::io::Cursor;
use tablecraft_ace::{
    Ace16WritePreview, EditError, InspectError, NativeImage, SlotScalarUpdate, check_table,
    check_table_constraints, check_table_long_values, read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

#[path = "default_long_value_updates/api.rs"]
mod api;

#[path = "../../../engines/tablecraft-ace/tests/ace16_long_value_updates_contract/forwarding_case.rs"]
mod forwarding;
