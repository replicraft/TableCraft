//! Independent parent/list models for explicitly typed INNER/CROSS query results.
#[path = "typed_multivalue_join_contract/boundaries.rs"]
mod boundaries;
#[path = "typed_multivalue_join_contract/namespaces.rs"]
mod namespaces;
use tablecraft_testkit::support::query::multivalue as support;
#[path = "typed_multivalue_join_contract/values.rs"]
mod values;
