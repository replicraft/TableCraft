#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
mod support;

#[path = "ace16_variable_updates_contract/model.rs"]
mod model;
#[allow(dead_code)]
#[path = "ace16_page_updates_contract/model.rs"]
mod page_model;
#[path = "ace16_variable_updates_contract/values.rs"]
mod values;

use model::*;
use page_model::{budget, capture, read, record};
use tablecraft_ace::{EditError, InspectError, SlotScalarUpdate, WriteBlocker};
use tablecraft_core::{limits::ResourceBudget, values::Value};
