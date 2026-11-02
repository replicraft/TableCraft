#![cfg(feature = "ace16-write-preview")]

use tablecraft_testkit::fixtures::access::mutation_fixture as fixture;
#[path = "foreign_updates/fixture.rs"]
mod foreign_fixture;
use tablecraft_testkit::fixtures::access as support;
use tablecraft_testkit::fixtures::access::index_fixture;
#[allow(dead_code)]
use tablecraft_testkit::fixtures::access::index_tuples::numeric_fixture;

use fixture as base;

#[path = "ace16_delete_contract/boundaries.rs"]
mod boundaries;
#[path = "ace16_delete_contract/model.rs"]
mod model;
#[path = "ace16_delete_contract/values.rs"]
mod values;
