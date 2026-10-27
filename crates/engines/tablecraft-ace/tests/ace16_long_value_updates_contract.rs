#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
mod support;

#[path = "ace16_long_value_updates_contract/census.rs"]
mod census;
#[path = "ace16_long_value_updates_contract/guards.rs"]
mod guards;
#[path = "ace16_long_value_updates_contract/model.rs"]
mod model;
#[path = "ace16_long_value_updates_contract/values.rs"]
mod values;

use model::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    Ace16WritePreview, EditError, InspectError, LongValueCheckStatus, NativeImage,
    SlotScalarUpdate, check_table, check_table_constraints, check_table_long_values, read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
