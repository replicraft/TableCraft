#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
#[allow(dead_code)]
#[path = "ace16_nonroot_split_int32_contract/model.rs"]
mod model;
#[allow(dead_code)]
#[path = "ace16_split_int32_contract/model.rs"]
mod root_model;
mod support;

use model::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{EditError, InspectError, Int32Update, check_table, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

#[path = "ace16_parent_split_int32/expected.rs"]
mod expected;
#[path = "ace16_parent_split_int32/values.rs"]
mod values;

use expected::{parent_expected, parent_source};
