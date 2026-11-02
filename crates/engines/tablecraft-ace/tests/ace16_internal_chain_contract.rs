#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "ace16_delete_contraction_contract/model.rs"]
mod contraction_model;
use tablecraft_testkit::fixtures::access as support;
#[allow(dead_code)]
use tablecraft_testkit::fixtures::access::index_fixture;
#[allow(dead_code)]
#[path = "ace16_delete_tree_contract/model.rs"]
mod tree_model;

#[path = "ace16_internal_chain_contract/boundaries.rs"]
mod boundaries;
#[path = "ace16_internal_chain_contract/model.rs"]
mod model;
#[path = "ace16_internal_chain_contract/values.rs"]
mod values;
