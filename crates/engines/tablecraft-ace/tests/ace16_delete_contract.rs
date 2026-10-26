#![cfg(feature = "ace16-write-preview")]

#[path = "mutation/fixture.rs"]
mod fixture;
#[path = "foreign_updates/fixture.rs"]
mod foreign_fixture;
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
#[allow(dead_code)]
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
mod support;

use fixture as base;

#[path = "ace16_delete_contract/boundaries.rs"]
mod boundaries;
#[path = "ace16_delete_contract/model.rs"]
mod model;
#[path = "ace16_delete_contract/values.rs"]
mod values;
