//! Shared literal typed fixtures for private request/index certification tests.
use super::{fixture, index_fixture};
use crate::{FixedScalarUpdate, IndexCheckStatus, check_table, read_table};
use std::io::Cursor;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

#[allow(dead_code)]
#[path = "../../tests/ace16_fixed_split_contract/model.rs"]
pub(crate) mod model;
#[allow(dead_code)]
#[path = "../../tests/ace16_split_int32_contract/model.rs"]
mod root_model;
