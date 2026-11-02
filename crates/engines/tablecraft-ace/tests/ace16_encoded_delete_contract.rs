#![cfg(feature = "ace16-write-preview")]

use tablecraft_testkit::fixtures::access as support;
#[allow(dead_code)]
use tablecraft_testkit::fixtures::access::index_fixture;
use tablecraft_testkit::fixtures::access::mutation_fixture as fixture;

#[path = "ace16_encoded_delete_contract/boundaries.rs"]
mod boundaries;
#[path = "ace16_encoded_delete_contract/model.rs"]
mod model;
#[path = "ace16_encoded_delete_contract/values.rs"]
mod values;
