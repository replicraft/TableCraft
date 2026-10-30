#[allow(dead_code)]
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::mutation_fixture as fixture;

#[cfg(feature = "ace16-write-preview")]
#[allow(dead_code)]
#[path = "ace16_long_value_updates_contract/model.rs"]
mod old_model;

#[path = "ace16_mixed_long_value_maps_contract/model.rs"]
mod model;
#[path = "ace16_mixed_long_value_maps_contract/storage.rs"]
mod storage;
#[cfg(feature = "ace16-write-preview")]
#[path = "ace16_mixed_long_value_maps_contract/values.rs"]
mod values;

use model::*;
#[cfg(feature = "ace16-write-preview")]
use std::cell::Cell;
use std::io::Cursor;
#[cfg(feature = "ace16-write-preview")]
use tablecraft_ace::{Ace16WritePreview, EditError, NativeImage, SlotScalarUpdate, WriteBlocker};
use tablecraft_ace::{
    InspectError, LongValueCheckStatus, check_table, check_table_constraints,
    check_table_long_values, read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
