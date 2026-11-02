#![cfg(feature = "ace16-write-preview")]

use tablecraft_testkit::fixtures::access as support;
#[allow(dead_code)]
use tablecraft_testkit::fixtures::access::index_fixture;
use tablecraft_testkit::fixtures::access::mutation_fixture as fixture;

#[path = "ace16_split_int32_contract/guards.rs"]
mod guards;
#[path = "ace16_split_int32_contract/model.rs"]
mod model;
#[path = "ace16_split_int32_contract/values.rs"]
mod values;

use model::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, IndexCheckStatus, InspectError, Int32Update, check_table, read_table_index,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
