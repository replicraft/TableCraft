#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
use tablecraft_testkit::access as support;

#[path = "ace16_page_updates_contract/model.rs"]
mod model;
#[path = "ace16_page_updates_contract/values.rs"]
mod values;

use model::*;
use tablecraft_ace::{EditError, InspectError, SlotScalarUpdate, WriteBlocker};
use tablecraft_core::{limits::ResourceBudget, values::Value};
