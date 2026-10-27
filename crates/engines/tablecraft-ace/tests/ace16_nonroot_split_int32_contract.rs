#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
#[allow(dead_code)]
#[path = "ace16_split_int32_contract/model.rs"]
mod root_model;
mod support;

#[path = "ace16_nonroot_split_int32_contract/guards.rs"]
mod guards;
#[path = "ace16_nonroot_split_int32_contract/model.rs"]
mod model;
#[path = "ace16_nonroot_split_int32_contract/values.rs"]
mod values;

use model::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{EditError, InspectError, Int32Update, check_table, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
