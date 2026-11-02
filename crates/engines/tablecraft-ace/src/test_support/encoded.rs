//! Shared independent encoded-key fixtures; no mutation admission is implied.
use tablecraft_testkit::fixtures::access as support;
use tablecraft_testkit::fixtures::access::index_fixture;
use tablecraft_testkit::fixtures::access::mutation_fixture as fixture;

#[allow(dead_code)]
#[path = "../../tests/ace16_encoded_delete_contract/model.rs"]
pub(crate) mod model;

pub(crate) use model as encoded_model;

#[allow(dead_code)]
#[path = "../../tests/ace16_encoded_split_contract/model.rs"]
pub(crate) mod split_model;
