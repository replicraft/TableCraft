#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
use tablecraft_testkit::fixtures::access::index_fixture;
use tablecraft_testkit::fixtures::access::mutation_fixture as fixture;
#[allow(dead_code)]
#[path = "ace16_split_int32_contract/model.rs"]
mod root_model;
use tablecraft_testkit::fixtures::access as support;

#[path = "ace16_fixed_split_contract/model.rs"]
mod model;
#[allow(dead_code)]
#[path = "ace16_repeated_fixed_split_contract/model.rs"]
mod repeated_model;
#[path = "ace16_fixed_split_contract/values.rs"]
mod values;

use model as fixture_model;

use model::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, FixedScalarUpdate, IndexCheckStatus, InspectError, Int32Update, WriteBlocker,
    check_table, read_table,
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;
