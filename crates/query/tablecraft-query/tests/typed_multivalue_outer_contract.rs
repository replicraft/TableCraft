//! Independent whole-list outer-join presence and error contracts.
#[path = "typed_multivalue_outer_contract/boundaries.rs"]
mod boundaries;
use tablecraft_testkit::support::query::multivalue as support;
#[path = "typed_multivalue_outer_contract/values.rs"]
mod values;
