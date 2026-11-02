#[cfg(feature = "ace16-write-preview")]
#[allow(dead_code)]
#[path = "ace16_encoded_delete_contract/model.rs"]
mod encoded_model;
use tablecraft_testkit::fixtures::access as support;
#[allow(dead_code)]
use tablecraft_testkit::fixtures::access::index_fixture;
use tablecraft_testkit::fixtures::access::mutation_fixture as fixture;

#[path = "french_legacy_reads/boundaries.rs"]
mod boundaries;
#[path = "french_legacy_reads/model.rs"]
mod model;
#[path = "french_legacy_reads/values.rs"]
mod values;
