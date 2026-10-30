#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "ace16_encoded_delete_contract/model.rs"]
mod encoded_model;
use tablecraft_testkit::access::mutation_fixture as fixture;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
use tablecraft_testkit::access as support;

#[path = "ace16_ignored_delete_contract/boundaries.rs"]
mod boundaries;
#[path = "ace16_ignored_delete_contract/model.rs"]
mod model;
#[path = "ace16_ignored_delete_contract/values.rs"]
mod values;
