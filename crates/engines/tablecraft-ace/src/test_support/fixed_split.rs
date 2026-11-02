//! Shared literal typed fixtures for private request/index certification tests.
use crate::{FixedScalarUpdate, IndexCheckStatus, check_table, read_table};
use std::io::Cursor;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access as support;
use tablecraft_testkit::fixtures::access::index_fixture;
use tablecraft_testkit::fixtures::access::mutation_fixture as fixture;

#[allow(dead_code)]
#[path = "../../tests/ace16_fixed_split_contract/model.rs"]
pub(crate) mod model;
#[allow(dead_code)]
#[path = "../../tests/ace16_split_int32_contract/model.rs"]
mod root_model;
