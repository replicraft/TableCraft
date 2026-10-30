#![cfg(feature = "ace16-write-preview")]

use tablecraft_testkit::access as support;
use tablecraft_testkit::access::mutation_fixture as fixture;

#[path = "ace16_page_updates_contract/model.rs"]
mod model;
#[path = "ace16_page_updates_contract/values.rs"]
mod values;

use model::*;
use tablecraft_ace::{EditError, InspectError, SlotScalarUpdate, WriteBlocker};
use tablecraft_core::{limits::ResourceBudget, values::Value};
