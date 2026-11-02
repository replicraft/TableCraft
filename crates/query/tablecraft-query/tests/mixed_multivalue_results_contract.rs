//! Independent complete-list plus displayed-child cardinality contracts.
#[path = "mixed_multivalue_results_contract/alignment.rs"]
mod alignment;
#[path = "mixed_multivalue_results_contract/boundaries.rs"]
mod boundaries;
use tablecraft_testkit::support::query::multivalue as support;
#[path = "mixed_multivalue_results_contract/values.rs"]
mod values;
