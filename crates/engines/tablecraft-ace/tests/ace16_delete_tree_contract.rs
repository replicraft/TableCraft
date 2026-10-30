#![cfg(feature = "ace16-write-preview")]

use tablecraft_testkit::access::mutation_fixture as fixture;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
#[allow(dead_code)]
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
use tablecraft_testkit::access as support;

use fixture as base;

#[path = "ace16_delete_tree_contract/boundaries.rs"]
mod boundaries;
#[path = "ace16_delete_tree_contract/model.rs"]
mod model;
#[path = "ace16_delete_tree_contract/values.rs"]
mod values;

#[path = "ace16_delete_tree_contract/composite.rs"]
mod composite;
