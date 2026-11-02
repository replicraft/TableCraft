#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
use tablecraft_testkit::fixtures::access::index_fixture;
use tablecraft_testkit::fixtures::access::mutation_fixture as fixture;
#[allow(dead_code)]
#[path = "ace16_nonroot_split_int32_contract/model.rs"]
mod old_model;
#[allow(dead_code)]
#[path = "ace16_split_int32_contract/model.rs"]
mod root_model;
use tablecraft_testkit::fixtures::access as support;

#[path = "ace16_internal_parent_split_int32_contract/model.rs"]
mod model;
#[path = "ace16_internal_parent_split_int32_contract/values.rs"]
mod values;

use model::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{EditError, InspectError, Int32Update, check_table, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
