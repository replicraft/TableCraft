#![cfg(feature = "ace16-write-preview")]

use tablecraft_testkit::access::mutation_fixture as fixture;
#[allow(dead_code)]
#[path = "ace16_restrict_delete_contract/model.rs"]
mod restrict_model;
#[allow(dead_code)]
#[path = "foreign_updates/fixture.rs"]
mod self_fixture;
use fixture as base;
use tablecraft_testkit::access as support;

#[path = "ace16_cascading_delete_contract/boundaries.rs"]
mod boundaries;
#[path = "ace16_cascading_delete_contract/model.rs"]
mod model;
#[path = "ace16_cascading_delete_contract/values.rs"]
mod values;
