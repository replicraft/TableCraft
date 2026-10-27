#[cfg(feature = "ace16-write-preview")]
#[allow(dead_code)]
#[path = "ace16_encoded_delete_contract/model.rs"]
mod encoded_model;
#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
mod support;

#[path = "french_legacy_reads/boundaries.rs"]
mod boundaries;
#[path = "french_legacy_reads/model.rs"]
mod model;
#[path = "french_legacy_reads/values.rs"]
mod values;
