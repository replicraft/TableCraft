#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
mod support;

#[path = "ace16_encoded_delete_contract/boundaries.rs"]
mod boundaries;
#[path = "ace16_encoded_delete_contract/model.rs"]
mod model;
#[path = "ace16_encoded_delete_contract/values.rs"]
mod values;
