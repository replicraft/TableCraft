#![cfg(feature = "ace16-write-preview")]

#[path = "ace16_restrict_delete_contract/boundaries.rs"]
mod boundaries;
#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[path = "ace16_restrict_delete_contract/model.rs"]
mod model;
#[allow(dead_code)]
#[path = "foreign_updates/fixture.rs"]
mod self_fixture;
use fixture as base;
mod support;
#[path = "ace16_restrict_delete_contract/values.rs"]
mod values;
