//! Shared independent encoded-key fixtures; no mutation admission is implied.
use super::{fixture, index_fixture};
use tablecraft_testkit::access as support;

#[allow(dead_code)]
#[path = "../../tests/ace16_encoded_delete_contract/model.rs"]
pub(crate) mod model;

pub(crate) use model as encoded_model;

#[allow(dead_code)]
#[path = "../../tests/ace16_encoded_split_contract/model.rs"]
pub(crate) mod split_model;
