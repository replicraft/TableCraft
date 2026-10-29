#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "ace16_cascading_delete_contract/model.rs"]
mod cascade_model;
#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "ace16_restrict_delete_contract/model.rs"]
mod restrict_model;
#[path = "ace16_cascade_key_facts_contract/tuple.rs"]
mod tuple;
use tablecraft_testkit::access as support;

#[path = "ace16_set_null_delete_contract/model.rs"]
mod model;
#[path = "ace16_set_null_delete_contract/role_boundary.rs"]
mod role_boundary;
#[path = "ace16_set_null_delete_contract/trees.rs"]
mod trees;
#[path = "ace16_set_null_delete_contract/values.rs"]
mod values;
