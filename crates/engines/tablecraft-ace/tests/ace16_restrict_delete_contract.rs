#![cfg(feature = "ace16-write-preview")]

#[path = "ace16_restrict_delete_contract/boundaries.rs"]
mod boundaries;
use tablecraft_testkit::access::mutation_fixture as fixture;
#[path = "ace16_restrict_delete_contract/model.rs"]
mod model;
#[allow(dead_code)]
#[path = "foreign_updates/fixture.rs"]
mod self_fixture;
use fixture as base;
use tablecraft_testkit::access as support;
#[path = "ace16_restrict_delete_contract/values.rs"]
mod values;
