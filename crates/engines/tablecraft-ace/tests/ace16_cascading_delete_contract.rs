#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "ace16_restrict_delete_contract/model.rs"]
mod restrict_model;
#[allow(dead_code)]
#[path = "foreign_updates/fixture.rs"]
mod self_fixture;
mod support;
use fixture as base;

#[path = "ace16_cascading_delete_contract/boundaries.rs"]
mod boundaries;
#[path = "ace16_cascading_delete_contract/model.rs"]
mod model;
#[path = "ace16_cascading_delete_contract/values.rs"]
mod values;
