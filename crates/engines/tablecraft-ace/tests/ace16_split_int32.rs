#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
use tablecraft_testkit::access as support;

#[path = "ace16_split_int32/model.rs"]
mod model;
#[path = "ace16_split_int32/values.rs"]
mod values;

use model::*;
use tablecraft_ace as engine;
use tablecraft_ace::{
    EditError, IndexConstraintViolation, InspectError, Int32Update, WriteBlocker,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
