//! SOURCE-only witnesses for authoritative INDEX-owned claims on ordinary DATA.
#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
#[path = "ace16_fixed_index_body_ownership/model.rs"]
mod model;
#[path = "ace16_fixed_index_body_ownership/source.rs"]
mod source;

use tablecraft_testkit::access as support;

#[path = "ace16_fixed_index_body_ownership/selected_orphan.rs"]
mod selected_orphan;

#[cfg(feature = "ace16-write-preview")]
#[path = "ace16_fixed_index_body_ownership/probe.rs"]
mod probe;
