//! Independent retained-list origins, element policies and read-barrier contracts.
#[path = "derived_multivalue_members_contract/boundaries.rs"]
mod boundaries;
#[path = "derived_multivalue_members_contract/origins.rs"]
mod origins;
#[path = "derived_multivalue_members_contract/policies.rs"]
mod policies;
use tablecraft_testkit::support::query::multivalue as support;
#[path = "derived_multivalue_members_contract/wide.rs"]
mod wide;
